#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlaybackOptions {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub relative_volume: Option<f32>,
}

impl PlaybackOptions {
    pub fn validate(self, duration_ms: Option<u64>) -> Result<(), String> {
        if let Some(db) = self.relative_volume
            && (!db.is_finite() || !(-15.0..=15.0).contains(&db))
        {
            return Err("Relative volume must be between -15 and +15 dB.".into());
        }
        let start = self.start_ms.unwrap_or(0);
        if self.end_ms.is_some_and(|end| start >= end) {
            return Err("Crop Start must be before End.".into());
        }
        if let Some(duration) = duration_ms {
            if start >= duration {
                return Err("Crop Start must be before the end of the audio file.".into());
            }
            if self.end_ms.is_some_and(|end| end > duration) {
                return Err("Crop End is beyond the end of the audio file.".into());
            }
        }
        Ok(())
    }

    pub(crate) fn gain(self) -> f32 {
        10.0_f32.powf(self.relative_volume.unwrap_or(0.0) / 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_gain_order_and_known_duration_without_mutating_options() {
        let invalid = [
            PlaybackOptions {
                relative_volume: Some(f32::NAN),
                ..Default::default()
            },
            PlaybackOptions {
                relative_volume: Some(f32::INFINITY),
                ..Default::default()
            },
            PlaybackOptions {
                relative_volume: Some(15.5),
                ..Default::default()
            },
            PlaybackOptions {
                relative_volume: Some(-15.5),
                ..Default::default()
            },
            PlaybackOptions {
                end_ms: Some(0),
                ..Default::default()
            },
            PlaybackOptions {
                start_ms: Some(50),
                end_ms: Some(50),
                ..Default::default()
            },
            PlaybackOptions {
                start_ms: Some(51),
                end_ms: Some(50),
                ..Default::default()
            },
            PlaybackOptions {
                start_ms: Some(100),
                ..Default::default()
            },
            PlaybackOptions {
                end_ms: Some(101),
                ..Default::default()
            },
        ];
        for options in invalid {
            assert!(options.validate(Some(100)).is_err(), "accepted {options:?}");
        }
        for options in [
            PlaybackOptions::default(),
            PlaybackOptions {
                start_ms: Some(99),
                ..Default::default()
            },
            PlaybackOptions {
                end_ms: Some(100),
                ..Default::default()
            },
            PlaybackOptions {
                start_ms: Some(1),
                end_ms: Some(100),
                relative_volume: Some(-15.0),
            },
        ] {
            assert!(options.validate(Some(100)).is_ok());
        }
        assert!(
            PlaybackOptions {
                start_ms: Some(100),
                ..Default::default()
            }
            .validate(None)
            .is_ok()
        );
    }

    #[test]
    fn converts_decibels_to_amplitude() {
        assert_eq!(PlaybackOptions::default().gain(), 1.0);
        for (db, expected) in [(-15.0, 0.17782794), (0.0, 1.0), (15.0, 5.623413)] {
            let options = PlaybackOptions {
                relative_volume: Some(db),
                ..Default::default()
            };
            assert!((options.gain() - expected).abs() < 0.00001);
        }
    }
}
