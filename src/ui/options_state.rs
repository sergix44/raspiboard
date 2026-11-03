use raspiboard::{
    audio::{PlaybackOptions, waveform::Waveform},
    storage::Button,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(super) struct OptionsState {
    pub generation: u64,
    pub cancel: Option<Arc<AtomicBool>>,
    pub waveform: Option<Arc<Waveform>>,
    pub loading: bool,
    pub analysis_error: Option<String>,
    pub start: String,
    pub end: String,
    pub crop_error: Option<String>,
    pub color_picker: bool,
}

impl OptionsState {
    pub fn reset(&mut self, button: &Button) {
        self.cancel_analysis();
        self.waveform = None;
        self.analysis_error = None;
        self.start = format_time(button.start_ms);
        self.end = format_time(button.end_ms);
        self.crop_error = crop_values(&self.start, &self.end, None).err();
        self.color_picker = false;
    }

    pub fn cancel_analysis(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.generation = self.generation.wrapping_add(1);
        self.loading = false;
    }

    pub fn duration(&self) -> Option<u64> {
        self.waveform.as_ref().map(|w| w.duration_ms)
    }
}

impl Drop for OptionsState {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
    }
}

pub(super) fn parse_time(value: &str) -> Result<Option<u64>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let invalid =
        || "Enter seconds with up to three decimal places (for example 1.250).".to_string();
    let (whole, fraction) = value
        .split_once('.')
        .map_or((value, None), |(a, b)| (a, Some(b)));
    if (whole.is_empty() && fraction.is_none()) || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    let seconds = if whole.is_empty() {
        0
    } else {
        whole.parse::<u64>().map_err(|_| invalid())?
    };
    let fraction = if let Some(fraction) = fraction {
        if fraction.is_empty()
            || fraction.len() > 3
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid());
        }
        fraction.parse::<u64>().map_err(|_| invalid())? * 10_u64.pow(3 - fraction.len() as u32)
    } else {
        0
    };
    seconds
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add(fraction))
        .map(Some)
        .ok_or_else(invalid)
}

pub(super) fn format_time(value: Option<u64>) -> String {
    value
        .map(|ms| {
            if ms % 1000 == 0 {
                (ms / 1000).to_string()
            } else {
                format!("{}.{:03}", ms / 1000, ms % 1000)
                    .trim_end_matches('0')
                    .to_string()
            }
        })
        .unwrap_or_default()
}

pub(super) fn crop_values(
    start: &str,
    end: &str,
    duration: Option<u64>,
) -> Result<(Option<u64>, Option<u64>), String> {
    let start = parse_time(start)?;
    let end = parse_time(end)?;
    PlaybackOptions {
        start_ms: start,
        end_ms: end,
        relative_volume: None,
    }
    .validate(duration)?;
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_are_exact_to_milliseconds_and_invalid_drafts_stay_invalid() {
        assert_eq!(parse_time(""), Ok(None));
        assert_eq!(parse_time("1.025"), Ok(Some(1025)));
        assert_eq!(parse_time(".5"), Ok(Some(500)));
        for invalid in [
            "-1",
            "NaN",
            "inf",
            "1.",
            "1.0001",
            "1e3",
            "18446744073709551615",
        ] {
            assert!(parse_time(invalid).is_err(), "{invalid}");
        }
        assert_eq!(format_time(Some(1025)), "1.025");
        assert_eq!(format_time(Some(1000)), "1");
    }

    #[test]
    fn crop_checks_optional_boundaries_and_source_duration() {
        assert_eq!(crop_values("", "2", Some(3000)), Ok((None, Some(2000))));
        assert_eq!(crop_values("1", "", Some(3000)), Ok((Some(1000), None)));
        assert!(crop_values("2", "1", Some(3000)).is_err());
        assert!(crop_values("3", "", Some(3000)).is_err());
        assert!(crop_values("", "4", Some(3000)).is_err());
        assert!(crop_values("", "0", Some(3000)).is_err());
        assert_eq!(crop_values("", "", None), Ok((None, None)));
    }
}
