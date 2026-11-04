use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
};

pub struct PlaybackPosition {
    frames: AtomicU64,
    rate: u32,
    start_ms: u64,
}

impl PlaybackPosition {
    pub fn new(rate: u32, start_ms: u64) -> Self {
        Self {
            frames: AtomicU64::new(0),
            rate,
            start_ms,
        }
    }

    pub fn position_ms(&self) -> u64 {
        let elapsed =
            u128::from(self.frames.load(Ordering::Relaxed)) * 1000 / u128::from(self.rate.max(1));
        self.start_ms
            .saturating_add(elapsed.min(u128::from(u64::MAX)) as u64)
    }
}

pub struct TransportFade {
    frames: usize,
    remaining: usize,
    last: [f32; 2],
}

impl TransportFade {
    pub fn new(rate: u32, paused: bool) -> Self {
        let frames = (rate / 200).max(1) as usize;
        Self {
            frames,
            remaining: if paused { 0 } else { frames },
            last: [0.0; 2],
        }
    }
}

pub struct SampleFeed {
    pub position: Arc<PlaybackPosition>,
    pub consumer: rtrb::Consumer<[f32; 2]>,
    pub eof: Arc<AtomicBool>,
    pub finished: Arc<AtomicBool>,
    pub volume: Arc<AtomicU8>,
    pub clip_gain: f32,
    pub enabled: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub fade: TransportFade,
}

impl SampleFeed {
    pub fn render(&mut self, bytes: &mut [u8]) -> usize {
        if self.finished.load(Ordering::Acquire) {
            return 0;
        }
        if !self.enabled.load(Ordering::Acquire) {
            bytes.fill(0);
            return bytes.len() / 8 * 8;
        }
        let gain = f32::from(self.volume.load(Ordering::Relaxed).min(100)) / 100.0;
        let mut written = 0;
        let mut consumed = 0;
        for frame in bytes.chunks_exact_mut(8) {
            // Completion must be observed before the queue read: the producer
            // may publish its last frame and EOF while this callback runs.
            let stopping = self.stop.load(Ordering::Acquire);
            let paused = self.paused.load(Ordering::Acquire);
            if self.fade.remaining == 0 {
                if stopping {
                    self.finished.store(true, Ordering::Release);
                    break;
                }
                if paused {
                    frame.fill(0);
                    written += 8;
                    continue;
                }
            }
            let eof = self.eof.load(Ordering::Acquire);
            let samples = match self.consumer.pop() {
                Ok(samples) => {
                    consumed += 1;
                    samples
                }
                Err(_) if stopping || paused => self.fade.last,
                Err(_) if eof => {
                    self.finished.store(true, Ordering::Release);
                    break;
                }
                Err(_) => [0.0; 2],
            };
            self.fade.last = samples;
            let envelope = if stopping || paused {
                self.fade.remaining -= 1;
                self.fade.remaining as f32 / self.fade.frames as f32
            } else {
                self.fade.remaining = (self.fade.remaining + 1).min(self.fade.frames);
                self.fade.remaining as f32 / self.fade.frames as f32
            };
            for (sample, output) in samples.into_iter().zip(frame.chunks_exact_mut(4)) {
                let sample = if sample.is_finite() {
                    (sample * (self.clip_gain * gain * envelope)).clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                output.copy_from_slice(&sample.to_le_bytes());
            }
            written += 8;
            if stopping && self.fade.remaining == 0 {
                self.finished.store(true, Ordering::Release);
                break;
            }
        }
        self.position.frames.fetch_add(consumed, Ordering::Relaxed);
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn a_seek_started_while_paused_stays_silent_and_preserves_position_until_resume() {
        let (mut producer, mut feed) = feed(100);
        feed.fade = TransportFade::new(1000, true);
        feed.position = Arc::new(PlaybackPosition::new(1000, 6000));
        feed.paused.store(true, Ordering::Release);
        producer.push([0.5; 2]).unwrap();
        producer.push([0.5; 2]).unwrap();
        let mut bytes = [255; 16];
        feed.render(&mut bytes);
        assert_eq!(bytes, [0; 16]);
        assert_eq!(feed.position.position_ms(), 6000);
        assert_eq!(feed.consumer.slots(), 2);
        feed.paused.store(false, Ordering::Release);
        feed.render(&mut bytes);
        assert!(f32::from_le_bytes(bytes[..4].try_into().unwrap()) > 0.0);
        assert_eq!(feed.position.position_ms(), 6002);
    }

    #[test]
    fn rapid_pause_resume_reverses_the_current_fade_without_a_jump() {
        let (mut producer, mut feed) = feed(100);
        feed.fade = TransportFade::new(1000, false);
        let mut bytes = [0; 8];
        for (paused, expected) in [(true, 0.8_f32), (false, 1.0), (true, 0.8), (true, 0.6)] {
            producer.push([1.0; 2]).unwrap();
            feed.paused.store(paused, Ordering::Release);
            feed.render(&mut bytes);
            assert_eq!(f32::from_le_bytes(bytes[..4].try_into().unwrap()), expected);
            assert!(!feed.finished.load(Ordering::Acquire));
        }
    }

    #[test]
    fn pause_preserves_queued_position_and_resume_fades_up() {
        let (mut producer, mut feed) = feed(100);
        feed.position = Arc::new(PlaybackPosition::new(1000, 500));
        feed.fade = TransportFade::new(400, false); // Two frames.
        for value in [0.5, 0.5, 0.25, 0.75] {
            producer.push([value; 2]).unwrap();
        }
        feed.paused.store(true, Ordering::Release);
        let mut bytes = [0; 16];
        assert_eq!(feed.render(&mut bytes), 16);
        assert_eq!(&bytes[..4], &0.25_f32.to_le_bytes());
        assert_eq!(&bytes[8..12], &0.0_f32.to_le_bytes());
        assert_eq!(feed.consumer.slots(), 2);
        assert_eq!(feed.position.position_ms(), 502);
        feed.eof.store(true, Ordering::Release);
        for _ in 0..3 {
            bytes.fill(255);
            assert_eq!(feed.render(&mut bytes), 16);
            assert_eq!(bytes, [0; 16]);
            assert_eq!(feed.consumer.slots(), 2);
            assert!(!feed.finished.load(Ordering::Acquire));
            assert_eq!(feed.position.position_ms(), 502);
        }
        feed.paused.store(false, Ordering::Release);
        assert_eq!(feed.render(&mut bytes), 16);
        assert_eq!(&bytes[..4], &0.125_f32.to_le_bytes());
        assert_eq!(&bytes[8..12], &0.75_f32.to_le_bytes());
        assert_eq!(feed.position.position_ms(), 504);
        assert_eq!(feed.render(&mut bytes), 0);
        assert!(feed.finished.load(Ordering::Acquire));
    }

    #[test]
    fn playback_position_counts_source_frames_from_crop_start_without_counting_silence() {
        let (mut producer, mut feed) = feed(0);
        feed.position = Arc::new(PlaybackPosition::new(2000, 750));
        producer.push([0.5; 2]).unwrap();
        producer.push([0.5; 2]).unwrap();
        feed.enabled.store(false, Ordering::Release);
        feed.render(&mut [0; 32]);
        assert_eq!(feed.position.position_ms(), 750);

        feed.enabled.store(true, Ordering::Release);
        feed.render(&mut [0; 32]);
        assert_eq!(feed.position.position_ms(), 751);
        feed.render(&mut [0; 32]);
        assert_eq!(feed.position.position_ms(), 751);

        feed.eof.store(true, Ordering::Release);
        feed.render(&mut [0; 32]);
        assert!(feed.finished.load(Ordering::Acquire));
        feed.render(&mut [0; 32]);
        assert_eq!(feed.position.position_ms(), 751);
    }

    #[test]
    fn stop_while_paused_finishes_without_consuming_more_samples() {
        let (mut producer, mut feed) = feed(100);
        feed.fade = TransportFade::new(200, false);
        producer.push([0.5; 2]).unwrap();
        producer.push([0.25; 2]).unwrap();
        feed.paused.store(true, Ordering::Release);
        feed.render(&mut [0; 8]);
        assert_eq!(feed.consumer.slots(), 1);
        feed.stop.store(true, Ordering::Release);
        assert_eq!(feed.render(&mut [0; 8]), 0);
        assert!(feed.finished.load(Ordering::Acquire));
        assert_eq!(feed.consumer.slots(), 1);
    }

    #[test]
    fn stop_fades_across_callbacks_even_when_the_queue_runs_dry() {
        let (mut producer, mut feed) = feed(100);
        feed.fade = TransportFade::new(1000, false); // Five frames.
        producer.push([0.5, -0.25]).unwrap();
        let mut bytes = [0; 8];
        feed.render(&mut bytes);
        feed.stop.store(true, Ordering::Release);
        feed.eof.store(true, Ordering::Release);
        for gain in [0.8, 0.6, 0.4, 0.2, 0.0] {
            assert_eq!(feed.render(&mut bytes), 8);
            let left = f32::from_le_bytes(bytes[..4].try_into().unwrap());
            let right = f32::from_le_bytes(bytes[4..].try_into().unwrap());
            assert!((left - 0.5 * gain).abs() < 1e-6);
            assert!((right + 0.25 * gain).abs() < 1e-6);
        }
        assert!(feed.finished.load(Ordering::Acquire));
        assert_eq!(feed.render(&mut bytes), 0);
    }

    #[test]
    fn concurrent_end_of_file_never_leaves_the_final_frame_queued() {
        for _ in 0..2000 {
            let (mut producer, mut feed) = feed(100);
            let eof = feed.eof.clone();
            std::thread::scope(|scope| {
                scope.spawn(move || {
                    producer.push([0.5, 0.5]).unwrap();
                    eof.store(true, Ordering::Release);
                });
                let mut bytes = [0; 8];
                while !feed.finished.load(Ordering::Acquire) {
                    feed.render(&mut bytes);
                }
            });
            assert_eq!(
                feed.consumer.slots(),
                0,
                "Completion was reported before consuming the final frame"
            );
        }
    }

    fn feed(volume: u8) -> (rtrb::Producer<[f32; 2]>, SampleFeed) {
        let (producer, consumer) = rtrb::RingBuffer::new(4);
        (
            producer,
            SampleFeed {
                position: Arc::new(PlaybackPosition::new(48000, 0)),
                consumer,
                eof: Arc::new(AtomicBool::new(false)),
                finished: Arc::new(AtomicBool::new(false)),
                volume: Arc::new(AtomicU8::new(volume)),
                clip_gain: 1.0,
                enabled: Arc::new(AtomicBool::new(true)),
                stop: Arc::new(AtomicBool::new(false)),
                paused: Arc::new(AtomicBool::new(false)),
                fade: TransportFade::new(48000, false),
            },
        )
    }

    #[test]
    fn scales_stereo_samples_and_fills_underruns_with_silence() {
        let (mut producer, mut feed) = feed(50);
        producer.push([1.0, -0.5]).unwrap();
        let mut bytes = [255; 16];
        assert_eq!(feed.render(&mut bytes), 16);
        assert_eq!(&bytes[..4], &0.5_f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &(-0.25_f32).to_le_bytes());
        assert_eq!(&bytes[8..], &[0; 8]);
        assert!(!feed.finished.load(Ordering::Acquire));
    }

    #[test]
    fn waiting_for_routing_outputs_silence_without_consuming_the_clip() {
        let (mut producer, mut feed) = feed(100);
        producer.push([0.5, 0.25]).unwrap();
        feed.eof.store(true, Ordering::Release);
        feed.enabled.store(false, Ordering::Release);
        let mut bytes = [255; 8];
        assert_eq!(feed.render(&mut bytes), 8);
        assert_eq!(bytes, [0; 8]);
        assert_eq!(feed.consumer.slots(), 1);
        assert!(!feed.finished.load(Ordering::Acquire));
        feed.enabled.store(true, Ordering::Release);
        feed.render(&mut bytes);
        assert_eq!(&bytes[..4], &0.5_f32.to_le_bytes());
    }

    #[test]
    fn end_of_file_preserves_last_frame_and_reports_completion() {
        let (mut producer, mut feed) = feed(100);
        producer.push([0.5, 0.25]).unwrap();
        feed.eof.store(true, Ordering::Release);
        let mut bytes = [0; 16];
        assert_eq!(feed.render(&mut bytes), 8);
        assert_eq!(&bytes[..4], &0.5_f32.to_le_bytes());
        assert!(feed.finished.load(Ordering::Acquire));
    }

    #[test]
    fn zero_master_and_zero_fade_silence_even_extreme_finite_samples() {
        for stopped in [false, true] {
            let (mut producer, mut feed) = feed(if stopped { 100 } else { 0 });
            feed.clip_gain = 5.623413;
            feed.fade = TransportFade::new(200, false);
            feed.stop.store(stopped, Ordering::Release);
            producer.push([f32::MAX, -f32::MAX]).unwrap();
            let mut bytes = [255; 8];
            feed.render(&mut bytes);
            assert_eq!(f32::from_le_bytes(bytes[..4].try_into().unwrap()), 0.0);
            assert_eq!(f32::from_le_bytes(bytes[4..].try_into().unwrap()), 0.0);
        }
    }

    #[test]
    fn combines_clip_and_master_gain_before_clamping_final_finite_output() {
        let (mut producer, mut feed) = feed(50);
        feed.clip_gain = 2.25;
        producer.push([0.8, -0.8]).unwrap();
        let mut bytes = [0; 8];
        feed.render(&mut bytes);
        assert!((f32::from_le_bytes(bytes[..4].try_into().unwrap()) - 0.9).abs() < 1e-6);
        assert!((f32::from_le_bytes(bytes[4..].try_into().unwrap()) + 0.9).abs() < 1e-6);
        feed.volume.store(100, Ordering::Release);
        producer.push([f32::MAX, -f32::MAX]).unwrap();
        feed.render(&mut bytes);
        assert_eq!(f32::from_le_bytes(bytes[..4].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(bytes[4..].try_into().unwrap()), -1.0);
    }

    #[test]
    fn clamps_only_after_applying_master_volume() {
        let (mut producer, mut feed) = feed(50);
        producer.push([2.0, -2.0]).unwrap();
        let mut bytes = [0; 8];
        feed.render(&mut bytes);
        assert_eq!(f32::from_le_bytes(bytes[..4].try_into().unwrap()), 1.0);
        assert_eq!(f32::from_le_bytes(bytes[4..].try_into().unwrap()), -1.0);
    }

    #[test]
    fn silences_non_finite_samples_and_obeys_live_volume() {
        let (mut producer, mut feed) = feed(100);
        producer.push([f32::NAN, f32::INFINITY]).unwrap();
        let mut bytes = [255; 8];
        feed.render(&mut bytes);
        assert_eq!(bytes, [0; 8]);
        producer.push([0.5, 0.5]).unwrap();
        feed.volume.store(0, Ordering::Release);
        feed.render(&mut bytes);
        assert_eq!(bytes, [0; 8]);
    }
}
