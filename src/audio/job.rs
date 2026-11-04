use std::{
    collections::VecDeque,
    fs::File,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use super::{PlaybackOptions, decode::ClipDecoder};

#[derive(Clone, Copy, Default)]
struct FrameRange {
    start: u64,
    end: Option<u64>,
}

impl FrameRange {
    fn new(options: PlaybackOptions, rate: u32) -> Self {
        let frame = |ms: u64| {
            ((u128::from(ms) * u128::from(rate)).div_ceil(1000)).min(u128::from(u64::MAX)) as u64
        };
        Self {
            start: frame(options.start_ms.unwrap_or(0)),
            end: options.end_ms.map(frame),
        }
    }
}

pub struct Decoded {
    pub rate: u32,
    pub clip_gain: f32,
    pub samples: rtrb::Consumer<[f32; 2]>,
    pub eof: Arc<AtomicBool>,
    pub result: mpsc::Receiver<Result<(), String>>,
}

pub struct DecodedOutputs {
    pub duration_ms: Option<u64>,
    pub remote: Option<Decoded>,
    pub local: Decoded,
}

pub struct DecodeJob {
    pub ready: mpsc::Receiver<Result<DecodedOutputs, String>>,
    cancel: Arc<AtomicBool>,
}

pub struct DurationJob {
    pub ready: mpsc::Receiver<Result<u64, String>>,
    cancel: Arc<AtomicBool>,
}

impl DurationJob {
    pub fn start(path: PathBuf) -> Result<Self, String> {
        let (sender, ready) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = cancel.clone();
        thread::Builder::new()
            .name("audio-duration".into())
            .spawn(move || {
                let result = super::waveform::analyze(&path, &cancelled)
                    .map(|waveform| waveform.duration_ms);
                let _ = sender.send(result);
            })
            .map_err(|error| format!("Cannot start audio duration analysis: {error}"))?;
        Ok(Self { ready, cancel })
    }
}

impl Drop for DurationJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl DecodeJob {
    pub fn start(
        path: PathBuf,
        remote_enabled: bool,
        options: PlaybackOptions,
    ) -> Result<Self, String> {
        options.validate(None)?;
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = cancel.clone();
        thread::Builder::new()
            .name("audio-decoder".into())
            .spawn(move || {
                let result = prepare(path, options, &cancelled);
                let (mut decoder, range) = match result {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                let eof = Arc::new(AtomicBool::new(false));
                let mut producers = Vec::new();
                let mut results = Vec::new();
                let mut output = || {
                    let (producer, samples) = rtrb::RingBuffer::new(16384);
                    let (result_tx, result) = mpsc::sync_channel(1);
                    producers.push(producer);
                    results.push(result_tx);
                    Decoded {
                        rate: decoder.rate,
                        clip_gain: options.gain(),
                        samples,
                        eof: eof.clone(),
                        result,
                    }
                };
                let remote = remote_enabled.then(&mut output);
                let local = output();
                let duration_ms = decoder.frames.map(|frames| {
                    (u128::from(frames) * 1000)
                        .div_ceil(u128::from(decoder.rate))
                        .min(u128::from(u64::MAX)) as u64
                });
                if ready_tx
                    .send(Ok(DecodedOutputs {
                        duration_ms,
                        remote,
                        local,
                    }))
                    .is_err()
                {
                    return;
                }
                let result = produce(&mut decoder, &mut producers, &cancelled, range);
                for sender in results {
                    let _ = sender.send(result.clone());
                }
                eof.store(true, Ordering::Release);
            })
            .map_err(|e| format!("Cannot start the audio decoder: {e}"))?;
        Ok(Self { ready, cancel })
    }
}

impl Drop for DecodeJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

fn prepare(
    path: PathBuf,
    options: PlaybackOptions,
    cancel: &AtomicBool,
) -> Result<(ClipDecoder, FrameRange), String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Audio decoding was cancelled.".into());
    }
    if !path
        .metadata()
        .map_err(|e| format!("Cannot read file: {e}"))?
        .is_file()
    {
        return Err("Choose a regular audio file.".into());
    }
    let file = File::open(&path).map_err(|e| format!("Cannot open file: {e}"))?;
    let extension = path.extension().and_then(|e| e.to_str());
    let mut decoder = ClipDecoder::open(Box::new(file), extension)?;
    if options.start_ms.is_none() && options.end_ms.is_none() {
        return Ok((decoder, FrameRange::default()));
    }
    let mut range = FrameRange::new(options, decoder.rate);
    if let Some(frames) = decoder.frames {
        let duration_ms = (u128::from(frames) * 1000)
            .div_ceil(u128::from(decoder.rate))
            .min(u128::from(u64::MAX)) as u64;
        options.validate(Some(duration_ms))?;
        range.end = range.end.map(|end| end.min(frames));
        if range.start > 0 && decoder.seek_to_frame(range.start).is_err() {
            let file = File::open(&path).map_err(|e| format!("Cannot open file: {e}"))?;
            decoder = ClipDecoder::open(Box::new(file), extension)?;
        }
        if cancel.load(Ordering::Acquire) {
            return Err("Audio decoding was cancelled.".into());
        }
        return Ok((decoder, range));
    }
    let required = range.end.unwrap_or(0).max(range.start.saturating_add(1));
    let channels = decoder.channels;
    let mut frames = 0_u64;
    while frames < required {
        if cancel.load(Ordering::Acquire) {
            return Err("Audio decoding was cancelled.".into());
        }
        let Some(samples) = decoder.next_samples()? else {
            let duration_ms = (u128::from(frames) * 1000)
                .div_ceil(u128::from(decoder.rate))
                .min(u128::from(u64::MAX)) as u64;
            options.validate(Some(duration_ms))?;
            if range.start >= frames {
                return Err("Crop Start must be before the end of the audio file.".into());
            }
            range.end = range.end.map(|end| end.min(frames));
            break;
        };
        frames = frames.saturating_add((samples.len() / channels) as u64);
    }
    if cancel.load(Ordering::Acquire) {
        return Err("Audio decoding was cancelled.".into());
    }
    let file = File::open(&path).map_err(|e| format!("Cannot open file: {e}"))?;
    Ok((ClipDecoder::open(Box::new(file), extension)?, range))
}

fn produce(
    decoder: &mut ClipDecoder,
    producers: &mut [rtrb::Producer<[f32; 2]>],
    cancel: &AtomicBool,
    range: FrameRange,
) -> Result<(), String> {
    let channels = decoder.channels;
    let fade_frames = (decoder.rate / 200).max(1) as usize;
    let mut tail = VecDeque::with_capacity(fade_frames + 1);
    let mut count = 0;
    let mut source_frame = decoder.first_frame;
    let mut decode_error = None;
    'decode: while !cancel.load(Ordering::Acquire) && range.end.is_none_or(|end| source_frame < end)
    {
        let samples = match decoder.next_samples() {
            Ok(Some(samples)) => samples,
            Ok(None) => break,
            Err(error) => {
                decode_error = Some(error);
                break;
            }
        };
        for sample in samples.chunks_exact(channels) {
            if cancel.load(Ordering::Acquire) {
                return Ok(());
            }
            if range.end.is_some_and(|end| source_frame >= end) {
                break 'decode;
            }
            source_frame += 1;
            if source_frame <= range.start {
                continue;
            }
            let gain = (count as f32 / fade_frames as f32).min(1.0);
            tail.push_back([sample[0] * gain, sample[channels - 1] * gain]);
            count += 1;
            if tail.len() > fade_frames {
                let frame = tail.pop_front().unwrap();
                if !push_frame(frame, producers, cancel) {
                    return Ok(());
                }
            }
        }
    }
    while let Some(mut frame) = tail.pop_front() {
        let gain = tail.len() as f32 / fade_frames as f32;
        for sample in &mut frame {
            *sample *= gain;
        }
        if !push_frame(frame, producers, cancel) {
            return Ok(());
        }
    }
    if let Some(error) = decode_error {
        Err(error)
    } else if !cancel.load(Ordering::Acquire)
        && (count == 0 || range.end.is_some_and(|end| source_frame < end))
    {
        Err("The crop is outside the audio file. Correct or clear the crop range.".into())
    } else {
        Ok(())
    }
}

fn push_frame(
    frame: [f32; 2],
    producers: &mut [rtrb::Producer<[f32; 2]>],
    cancel: &AtomicBool,
) -> bool {
    for producer in producers {
        loop {
            if cancel.load(Ordering::Acquire) {
                return false;
            }
            match producer.push(frame) {
                Ok(()) => break,
                Err(_) => thread::sleep(Duration::from_millis(2)),
            }
        }
    }
    true
}

#[cfg(test)]
mod tests;
