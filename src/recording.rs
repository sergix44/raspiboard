use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod encoder;
pub(crate) mod writer;

pub const SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingFormat {
    #[default]
    Flac,
    Wav,
    Ogg,
}

impl RecordingFormat {
    pub const ALL: [Self; 3] = [Self::Flac, Self::Wav, Self::Ogg];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
        }
    }
}

impl std::fmt::Display for RecordingFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Flac => "FLAC",
            Self::Wav => "WAV",
            Self::Ogg => "Ogg Vorbis",
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RecordingSettings {
    pub format: RecordingFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<PathBuf>,
}

#[cfg(test)]
mod tests;
