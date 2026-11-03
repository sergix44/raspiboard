use std::{
    collections::VecDeque,
    fs::File,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use super::decode::ClipDecoder;

const MAX_PEAKS: usize = 2048;
const CACHE_SIZE: usize = 16;
type CachedWaveforms = VecDeque<(CacheKey, Arc<Waveform>)>;
static CACHE: LazyLock<Mutex<CachedWaveforms>> = LazyLock::new(Mutex::default);

#[derive(PartialEq, Eq)]
struct CacheKey {
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
}

impl CacheKey {
    fn read(path: &Path) -> Option<Self> {
        let metadata = path.metadata().ok()?;
        Some(Self {
            path: path.into(),
            bytes: metadata.len(),
            modified: metadata.modified().ok()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Waveform {
    pub peaks: Vec<f32>,
    pub duration_ms: u64,
}

pub fn analyze(path: &Path, cancel: &AtomicBool) -> Result<Arc<Waveform>, String> {
    check_cancel(cancel)?;
    let key = CacheKey::read(path);
    if let Some(key) = &key {
        let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(index) = cache.iter().position(|(current, _)| current == key) {
            let entry = cache.remove(index).unwrap();
            let waveform = entry.1.clone();
            cache.push_back(entry);
            check_cancel(cancel)?;
            return Ok(waveform);
        }
    }
    let waveform = Arc::new(analyze_uncached(path, cancel)?);
    if let Some(key) = key
        && CacheKey::read(path).as_ref() == Some(&key)
    {
        let mut cache = CACHE.lock().unwrap_or_else(|error| error.into_inner());
        cache.retain(|(current, _)| current.path != key.path);
        cache.push_back((key, waveform.clone()));
        if cache.len() > CACHE_SIZE {
            cache.pop_front();
        }
    }
    Ok(waveform)
}

fn analyze_uncached(path: &Path, cancel: &AtomicBool) -> Result<Waveform, String> {
    check_cancel(cancel)?;
    let file = File::open(path).map_err(|error| format!("Cannot open audio: {error}"))?;
    let mut decoder = ClipDecoder::open(Box::new(file), path.extension().and_then(|e| e.to_str()))?;
    let (rate, channels) = (decoder.rate, decoder.channels);
    let mut summary = Summary::default();
    loop {
        check_cancel(cancel)?;
        let Some(samples) = decoder.next_samples()? else {
            break;
        };
        for block in samples.chunks(4096 * channels) {
            check_cancel(cancel)?;
            summary.extend(block, channels);
        }
    }
    check_cancel(cancel)?;
    let duration_ms = ((u128::from(summary.frames) * 1000).div_ceil(u128::from(rate))) as u64;
    Ok(Waveform {
        peaks: summary.finish(),
        duration_ms,
    })
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Waveform analysis cancelled.".into())
    } else {
        Ok(())
    }
}

struct Summary {
    peaks: Vec<f32>,
    frames: u64,
    bin_frames: u64,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            peaks: Vec::with_capacity(MAX_PEAKS),
            frames: 0,
            bin_frames: 1,
        }
    }
}

impl Summary {
    fn extend(&mut self, mut samples: &[f32], channels: usize) {
        while samples.len() >= channels {
            if self.frames / self.bin_frames == MAX_PEAKS as u64 {
                for index in 0..MAX_PEAKS / 2 {
                    self.peaks[index] = self.peaks[index * 2].max(self.peaks[index * 2 + 1]);
                }
                self.peaks.truncate(MAX_PEAKS / 2);
                self.bin_frames *= 2;
            }
            let index = (self.frames / self.bin_frames) as usize;
            let frames = (self.bin_frames - self.frames % self.bin_frames)
                .min((samples.len() / channels) as u64) as usize;
            let length = frames * channels;
            let peak = samples[..length]
                .iter()
                .copied()
                .filter(|value| value.is_finite())
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs().min(1.0)));
            if index == self.peaks.len() {
                self.peaks.push(peak);
            } else {
                self.peaks[index] = self.peaks[index].max(peak);
            }
            self.frames += frames as u64;
            samples = &samples[length..];
        }
    }

    fn finish(self) -> Vec<f32> {
        let count = self.peaks.len();
        if count == 0 {
            return Vec::new();
        }
        // The final streaming bin can be short. Rebin its true frame span so
        // displaying equal-width bars does not stretch the end of the clip.
        (0..count)
            .map(|index| {
                let start = index as u128 * u128::from(self.frames) / count as u128;
                let end = ((index + 1) as u128 * u128::from(self.frames)).div_ceil(count as u128);
                let first = (start / u128::from(self.bin_frames)) as usize;
                let last = ((end.saturating_sub(1)) / u128::from(self.bin_frames)) as usize;
                self.peaks[first..=last.min(count - 1)]
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, sync::atomic::AtomicBool};

    fn wav(channels: u16, rate: u32, samples: &[i16]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let size = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + size).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(channels.to_le_bytes());
        bytes.extend(rate.to_le_bytes());
        bytes.extend((rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend((channels * 2).to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(size.to_le_bytes());
        for sample in samples {
            bytes.extend(sample.to_le_bytes());
        }
        file.write_all(&bytes).unwrap();
        file
    }

    #[test]
    fn stereo_peaks_use_both_channels_and_duration_counts_frames() {
        let file = wav(2, 1000, &[0, 16384, -32768, 0, 0, 8192]);
        let result = analyze(file.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.duration_ms, 3);
        assert_eq!(result.peaks, vec![0.5, 1.0, 0.25]);
    }

    #[test]
    fn decimation_stays_bounded_and_keeps_tail_at_end_of_waveform() {
        let mut samples = vec![0_i16; 100_003];
        *samples.last_mut().unwrap() = 32767;
        let file = wav(1, 44100, &samples);
        let result = analyze(file.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.duration_ms, 2268);
        assert!(result.peaks.len() <= 2048);
        assert!(result.peaks.last().unwrap() > &0.99);
        assert!(
            result.peaks[..result.peaks.len() - 2]
                .iter()
                .all(|p| *p == 0.0)
        );
    }

    #[test]
    fn cancellation_precedes_file_access() {
        assert_eq!(
            analyze(
                std::path::Path::new("/nonexistent-audio"),
                &AtomicBool::new(true)
            )
            .unwrap_err(),
            "Waveform analysis cancelled."
        );
    }

    #[test]
    fn unreadable_audio_returns_inline_error() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(analyze(file.path(), &AtomicBool::new(false)).is_err());
    }

    #[test]
    fn opus_uses_the_same_decoder_registry_as_playback() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio/tone-opus.ogg");
        let result = analyze(&path, &AtomicBool::new(false)).unwrap();
        assert!((97..=125).contains(&result.duration_ms));
        assert!(result.peaks.iter().any(|peak| *peak > 0.01));
    }

    #[test]
    fn unchanged_waveforms_are_reused_and_file_edits_invalidate_the_cache() {
        let file = wav(1, 1000, &[0, 16384]);
        let first = analyze(file.path(), &AtomicBool::new(false)).unwrap();
        let second = analyze(file.path(), &AtomicBool::new(false)).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        let mut bytes = std::fs::read(file.path()).unwrap();
        let len = bytes.len();
        bytes[len - 2..].copy_from_slice(&32767_i16.to_le_bytes());
        std::fs::write(file.path(), bytes).unwrap();
        let previous = file.as_file().metadata().unwrap().modified().unwrap();
        file.as_file()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(previous + std::time::Duration::from_secs(1)),
            )
            .unwrap();
        let changed = analyze(file.path(), &AtomicBool::new(false)).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&first, &changed));
        assert!(changed.peaks[1] > 0.99);
        assert!(analyze(file.path(), &AtomicBool::new(true)).is_err());
    }

    #[test]
    fn waveform_summary_is_independent_of_decoder_packet_boundaries() {
        let samples: Vec<_> = (0..150_003).map(|n| (n % 1024) as f32 / 1024.0).collect();
        let mut whole = Summary::default();
        whole.extend(&samples, 1);
        let mut split = Summary::default();
        for block in samples.chunks(137) {
            split.extend(block, 1);
        }
        assert_eq!(whole.frames, split.frames);
        assert_eq!(whole.finish(), split.finish());
    }
}
