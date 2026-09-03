pub const STEM_COUNT: usize = 6;
pub const STEM_NAMES: [&str; STEM_COUNT] = ["vocals", "drums", "bass", "other", "guitar", "piano"];
pub const MODEL_NAME: &str = "htdemucs_6s";
// Both inference backends use the same converted safetensors and official
// 5c90dfd2/34c22ccb source identity. Keep the cache portable between machines.
pub const MODEL_REVISION: &str = "htdemucs_6s-5c90dfd2-34c22ccb-fast-v2";
