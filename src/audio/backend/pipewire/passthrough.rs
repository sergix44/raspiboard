use super::{OutputVolumes, graph::Graph, job::Decoded, player::Player, route::Route};
use crate::audio::Target;
use pipewire as pw;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use super::capture::Capture;
const RATE: u32 = crate::recording::SAMPLE_RATE;

pub(super) struct Passthrough {
    capture: Capture,
    player: Player,
    route: Route,
    started: Instant,
    active: bool,
}

impl Passthrough {
    pub fn new(
        core: &pw::core::CoreRc,
        graph: &Graph,
        source: &Target,
        target: Target,
        sequence: u64,
        volumes: &OutputVolumes,
    ) -> Result<Self, String> {
        let source = graph.resolve(source)?;
        let channels = graph.channels(source, "out")?.len();
        let route = Route::new(core, graph, target, sequence)?;
        let (producer, samples) = rtrb::RingBuffer::new(4096);
        let (_, result) = mpsc::channel();
        let player = Player::new(
            core,
            Some(&route.sink.name),
            Decoded {
                rate: RATE,
                clip_gain: 1.0,
                samples,
                eof: Arc::new(AtomicBool::new(false)),
                result,
            },
            volumes.remote.clone(),
            Arc::new(AtomicBool::new(false)),
        )?;
        let capture =
            Capture::application(core, source, channels, producer, player.enabled.clone())?;
        Ok(Self {
            capture,
            player,
            route,
            started: Instant::now(),
            active: false,
        })
    }

    pub fn tick(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<bool, String> {
        if let Some(error) = self.player.error.borrow_mut().take() {
            return Err(error);
        }
        let capture_ready = self.capture.prepare(core, graph)?;
        let route_ready = self.route.prepare(core, graph)?;
        let player_ready = match self.route.sink.id.get() {
            Some(sink) => graph.has_link(self.player.stream.node_id(), sink)?,
            None => false,
        };
        let ready = capture_ready && route_ready && player_ready;
        if self.active && !ready {
            return Err(
                "The passthrough connection was removed. Enable it again to reconnect.".into(),
            );
        }
        if !ready && self.started.elapsed() > Duration::from_secs(10) {
            return Err(
                "Passthrough routing timed out. Check the source and recording application.".into(),
            );
        }
        self.player.enabled.store(ready, Ordering::Release);
        self.active = ready;
        Ok(ready)
    }

    pub fn stop(self) -> Option<RetiringPassthrough> {
        self.player.stop.store(true, Ordering::Release);
        self.active.then_some(RetiringPassthrough {
            player: self.player,
            _route: self.route,
            since: Instant::now(),
        })
    }
}

pub(super) struct RetiringPassthrough {
    player: Player,
    _route: Route,
    since: Instant,
}

impl RetiringPassthrough {
    pub fn retain(&mut self) -> bool {
        if self.since.elapsed() >= Duration::from_secs(3) {
            return false;
        }
        if self.player.finished.load(Ordering::Acquire) && !self.player.draining {
            if self.player.stream.flush(true).is_err() {
                return false;
            }
            self.player.draining = true;
        }
        !self.player.drained.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::super::capture::queue_capture;
    use super::*;

    #[test]
    fn passthrough_live_frames_use_remote_gain_without_local_gain() {
        use crate::audio::buffer::{SampleFeed, TransportFade};
        let volumes = OutputVolumes::default();
        let (mut producer, consumer) = rtrb::RingBuffer::new(8);
        let mut feed = SampleFeed {
            consumer,
            eof: Arc::new(AtomicBool::new(false)),
            finished: Arc::new(AtomicBool::new(false)),
            volume: volumes.remote.clone(),
            clip_gain: 1.0,
            enabled: Arc::new(AtomicBool::new(true)),
            stop: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            fade: TransportFade::new(RATE),
        };
        let input: Vec<_> = [0.8_f32, -0.4]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        for (remote, local) in [(25, 100), (0, 100), (100, 0)] {
            volumes.set(remote, local);
            queue_capture(&input, 0, input.len(), 8, 2, &mut producer);
            let mut output = [0; 8];
            feed.render(&mut output);
            let left = f32::from_le_bytes(output[..4].try_into().unwrap());
            let right = f32::from_le_bytes(output[4..].try_into().unwrap());
            assert!((left - 0.8 * remote as f32 / 100.0).abs() < 0.00001);
            assert!((right + 0.4 * remote as f32 / 100.0).abs() < 0.00001);
        }
    }

    #[test]
    fn passthrough_capture_respects_offsets_sizes_strides_and_duplicates_mono() {
        let mut bytes = vec![255; 4];
        for sample in [0.25_f32, 99.0, -0.5, 99.0] {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(8);
        queue_capture(&bytes, 4, 16, 8, 1, &mut producer);
        assert_eq!(consumer.pop().unwrap(), [0.25; 2]);
        assert_eq!(consumer.pop().unwrap(), [-0.5; 2]);
        assert!(consumer.pop().is_err());
        queue_capture(&[], usize::MAX, 8, 8, 2, &mut producer);
        queue_capture(&bytes, 0, 8, 2, 2, &mut producer);
        queue_capture(&bytes, 0, 8, 8, 3, &mut producer);
        assert!(consumer.pop().is_err());
    }

    #[test]
    fn passthrough_capture_wraps_offsets_and_frames_in_circular_buffers() {
        let bytes: Vec<_> = [0.25_f32, -0.5, 0.75, -1.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(4);
        queue_capture(&bytes, 28, usize::MAX, 8, 2, &mut producer);
        assert_eq!(consumer.pop().unwrap(), [-1.0, 0.25]);
        assert_eq!(consumer.pop().unwrap(), [-0.5, 0.75]);
        assert!(consumer.pop().is_err());
    }

    #[test]
    fn passthrough_stereo_capture_is_bounded_and_never_waits_for_a_full_queue() {
        let bytes: Vec<_> = [0.25_f32, -0.5, 0.75, -1.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(1);
        queue_capture(&bytes, 0, usize::MAX, 0, 2, &mut producer);
        queue_capture(&bytes, 0, bytes.len(), 8, 2, &mut producer);
        assert_eq!(consumer.pop().unwrap(), [0.25, -0.5]);
        assert!(consumer.pop().is_err());
    }
}
