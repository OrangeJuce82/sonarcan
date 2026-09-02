const TARGET_LUFS: f64 = -16.0;
const PEAK_CEILING_DBFS: f64 = -1.0;
const ABSOLUTE_GATE_LUFS: f64 = -70.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LoudnessAnalysis {
    pub(crate) integrated_lufs: f32,
    pub(crate) true_peak: f32,
}

impl LoudnessAnalysis {
    pub(crate) const SILENCE: Self = Self {
        integrated_lufs: f32::NEG_INFINITY,
        true_peak: 0.0,
    };

    pub(crate) fn normalization_gain(self) -> f32 {
        if !self.integrated_lufs.is_finite() || self.true_peak <= 0.0 {
            return 1.0;
        }
        let loudness_gain = 10.0_f64.powf((TARGET_LUFS - self.integrated_lufs as f64) / 20.0);
        let peak_ceiling = 10.0_f64.powf(PEAK_CEILING_DBFS / 20.0);
        loudness_gain.min(peak_ceiling / self.true_peak as f64) as f32
    }
}

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn process(&mut self, sample: f64) -> f64 {
        let output = self.b0 * sample + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = sample;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

fn k_weighting(sample_rate: u32) -> (Biquad, Biquad) {
    let rate = sample_rate as f64;
    let shelf_frequency = 1_681.974_450_955_533;
    let shelf_q = 0.707_175_236_955_419_6;
    let shelf_gain = 3.999_843_853_973_347;
    let vh = 10.0_f64.powf(shelf_gain / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let k = (std::f64::consts::PI * shelf_frequency / rate).tan();
    let denominator = 1.0 + k / shelf_q + k * k;
    let shelf = Biquad {
        b0: (vh + vb * k / shelf_q + k * k) / denominator,
        b1: 2.0 * (k * k - vh) / denominator,
        b2: (vh - vb * k / shelf_q + k * k) / denominator,
        a1: 2.0 * (k * k - 1.0) / denominator,
        a2: (1.0 - k / shelf_q + k * k) / denominator,
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    };

    let high_pass_frequency = 38.135_470_876_024_44;
    let high_pass_q = 0.500_327_037_323_877_3;
    let k = (std::f64::consts::PI * high_pass_frequency / rate).tan();
    let denominator = 1.0 + k / high_pass_q + k * k;
    let high_pass = Biquad {
        b0: 1.0 / denominator,
        b1: -2.0 / denominator,
        b2: 1.0 / denominator,
        a1: 2.0 * (k * k - 1.0) / denominator,
        a2: (1.0 - k / high_pass_q + k * k) / denominator,
        x1: 0.0,
        x2: 0.0,
        y1: 0.0,
        y2: 0.0,
    };
    (shelf, high_pass)
}

pub(crate) fn analyze(samples: &[f32], channels: usize, sample_rate: u32) -> LoudnessAnalysis {
    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return LoudnessAnalysis::SILENCE;
    }
    let measured_channels = channels.min(2);
    let mut filters = (0..measured_channels)
        .map(|_| k_weighting(sample_rate))
        .collect::<Vec<_>>();
    let block_frames = ((sample_rate as usize * 400) / 1_000).max(1);
    let step_frames = ((sample_rate as usize * 100) / 1_000).max(1);
    let mut energy_window = vec![0.0_f64; block_frames];
    let mut window_length = 0_usize;
    let mut window_index = 0_usize;
    let mut window_sum = 0.0_f64;
    let mut frames_since_block = 0_usize;
    let mut block_energies = Vec::new();
    for frame in samples.chunks_exact(channels) {
        let mut energy = 0.0;
        for channel in 0..measured_channels {
            let (shelf, high_pass) = &mut filters[channel];
            let weighted = high_pass.process(shelf.process(frame[channel] as f64));
            energy += weighted * weighted;
        }
        if window_length < block_frames {
            energy_window[window_length] = energy;
            window_length += 1;
            window_sum += energy;
            if window_length == block_frames {
                block_energies.push(window_sum / block_frames as f64);
            }
        } else {
            window_sum += energy - energy_window[window_index];
            energy_window[window_index] = energy;
            window_index = (window_index + 1) % block_frames;
            frames_since_block += 1;
            if frames_since_block == step_frames {
                block_energies.push(window_sum.max(0.0) / block_frames as f64);
                frames_since_block = 0;
            }
        }
    }
    if block_energies.is_empty() {
        block_energies.push(window_sum / window_length.max(1) as f64);
    }

    let absolute_gate_energy = energy_from_lufs(ABSOLUTE_GATE_LUFS);
    let above_absolute = block_energies
        .iter()
        .copied()
        .filter(|energy| *energy >= absolute_gate_energy)
        .collect::<Vec<_>>();
    let integrated_lufs = if above_absolute.is_empty() {
        f64::NEG_INFINITY
    } else {
        let ungated = above_absolute.iter().sum::<f64>() / above_absolute.len() as f64;
        let relative_gate = energy_from_lufs(lufs_from_energy(ungated) - 10.0);
        let gated = above_absolute
            .iter()
            .copied()
            .filter(|energy| *energy >= relative_gate)
            .collect::<Vec<_>>();
        lufs_from_energy(gated.iter().sum::<f64>() / gated.len() as f64)
    };

    LoudnessAnalysis {
        integrated_lufs: integrated_lufs as f32,
        true_peak: oversampled_peak(samples, channels),
    }
}

fn lufs_from_energy(energy: f64) -> f64 {
    -0.691 + 10.0 * energy.max(f64::MIN_POSITIVE).log10()
}

fn energy_from_lufs(lufs: f64) -> f64 {
    10.0_f64.powf((lufs + 0.691) / 10.0)
}

// A four-point cubic interpolation catches common inter-sample overshoots while
// keeping import analysis dependency-free. The final limiter remains the safety boundary.
fn oversampled_peak(samples: &[f32], channels: usize) -> f32 {
    let frames = samples.len() / channels;
    let mut peak = samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    if frames < 4 {
        return peak;
    }
    for channel in 0..channels.min(2) {
        for frame in 1..frames - 2 {
            let p0 = samples[(frame - 1) * channels + channel];
            let p1 = samples[frame * channels + channel];
            let p2 = samples[(frame + 1) * channels + channel];
            let p3 = samples[(frame + 2) * channels + channel];
            for phase in 1..4 {
                let t = phase as f32 / 4.0;
                let value = 0.5
                    * ((2.0 * p1)
                        + (-p0 + p2) * t
                        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t);
                peak = peak.max(value.abs());
            }
        }
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_not_amplified() {
        let result = analyze(&vec![0.0; 48_000], 1, 48_000);
        assert_eq!(result, LoudnessAnalysis::SILENCE);
        assert_eq!(result.normalization_gain(), 1.0);
    }

    #[test]
    fn normalization_targets_minus_sixteen_lufs() {
        let analysis = LoudnessAnalysis {
            integrated_lufs: -22.0,
            true_peak: 0.25,
        };
        assert!((analysis.normalization_gain() - 1.995_262).abs() < 0.000_01);
    }

    #[test]
    fn normalization_never_exceeds_the_peak_ceiling() {
        let analysis = LoudnessAnalysis {
            integrated_lufs: -30.0,
            true_peak: 0.8,
        };
        let normalized_peak = analysis.normalization_gain() * analysis.true_peak;
        assert!((normalized_peak - 10.0_f32.powf(-1.0 / 20.0)).abs() < 0.000_01);
    }

    #[test]
    fn full_scale_stereo_sine_has_expected_loudness_range() {
        let rate = 48_000;
        let mut samples = Vec::with_capacity(rate as usize * 2);
        for frame in 0..rate {
            let sample = (2.0 * std::f32::consts::PI * 1_000.0 * frame as f32 / rate as f32).sin();
            samples.extend([sample, sample]);
        }
        let result = analyze(&samples, 2, rate);
        assert!(
            (-1.0..=0.2).contains(&result.integrated_lufs),
            "measured {} LUFS",
            result.integrated_lufs
        );
        assert!((0.99..=1.01).contains(&result.true_peak));
    }
}
