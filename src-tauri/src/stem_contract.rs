use serde::{Deserialize, Serialize};

pub const STEM_COUNT: usize = 4;
pub const STEM_NAMES: [&str; STEM_COUNT] = ["vocals", "drums", "bass", "other"];

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StemSeparationProfile {
    #[default]
    Fast,
    Hq,
}

impl StemSeparationProfile {
    pub const fn argument(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Hq => "hq",
        }
    }

    pub const fn model_name(self) -> &'static str {
        match self {
            Self::Fast => "HTDemucs 4 stems",
            Self::Hq => "SCNet Large by starrytong",
        }
    }

    pub const fn model_revision(self) -> &'static str {
        match self {
            Self::Fast => "htdemucs-955717e8-8726e21a-overlap25-v1",
            Self::Hq => "scnet-large-starrytong-v1.0.9-65900dfa-v1",
        }
    }
}
