use super::{
    graph::{APP_ID, Graph},
    player::audio_format,
    route::Links,
};
use crate::recording::{
    SAMPLE_RATE,
    writer::{BAD_CAPTURE, QUEUE_OVERFLOW},
};
use pipewire::{self as pw, properties::properties, spa};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

const RATE: u32 = SAMPLE_RATE;
// SPA_CHUNK_FLAG_EMPTY is not named by the Rust ChunkFlags wrapper.
const EMPTY_CHUNK_FLAG: i32 = 1 << 1;

struct CaptureData {
    samples: rtrb::Producer<[f32; 2]>,
    enabled: Arc<AtomicBool>,
    channels: usize,
    fault: Option<Arc<AtomicU8>>,
}

impl CaptureData {
    fn fail(&self, reason: u8) {
        if let Some(fault) = &self.fault {
            let _ = fault.compare_exchange(0, reason, Ordering::AcqRel, Ordering::Acquire);
        }
    }

    fn push(&mut self, bytes: &[u8], offset: usize, size: usize, stride: i32, flags: i32) {
        if size == 0 {
            return;
        }
        if !self.enabled.load(Ordering::Acquire)
            || self
                .fault
                .as_ref()
                .is_some_and(|fault| fault.load(Ordering::Acquire) != 0)
        {
            return;
        }
        if flags & spa::buffer::ChunkFlags::CORRUPTED.bits() != 0 || stride < 0 {
            self.fail(BAD_CAPTURE);
            return;
        }
        if flags & EMPTY_CHUNK_FLAG != 0 {
            let width = self.channels * 4;
            let stride = if stride == 0 { width } else { stride as usize };
            if stride < width {
                self.fail(BAD_CAPTURE);
                return;
            }
            for _ in 0..size / stride {
                if self.samples.push([0.0; 2]).is_err() {
                    self.fail(QUEUE_OVERFLOW);
                    break;
                }
            }
        } else if !queue_capture(
            bytes,
            offset,
            size,
            stride as usize,
            self.channels,
            &mut self.samples,
        ) {
            self.fail(QUEUE_OVERFLOW);
        }
    }
}

pub(super) struct Capture {
    links: Option<Links>,
    _listener: pw::stream::StreamListener<CaptureData>,
    stream: pw::stream::StreamRc,
    error: Rc<RefCell<Option<String>>>,
    source: u32,
}

impl Capture {
    pub fn application(
        core: &pw::core::CoreRc,
        source: u32,
        channels: usize,
        samples: rtrb::Producer<[f32; 2]>,
        enabled: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        Self::new(core, source, channels, samples, enabled, None)
    }

    pub fn recording_mix(
        core: &pw::core::CoreRc,
        source: u32,
        samples: rtrb::Producer<[f32; 2]>,
        enabled: Arc<AtomicBool>,
        fault: Arc<AtomicU8>,
    ) -> Result<Self, String> {
        Self::new(core, source, 2, samples, enabled, Some(fault))
    }

    fn new(
        core: &pw::core::CoreRc,
        source: u32,
        channels: usize,
        samples: rtrb::Producer<[f32; 2]>,
        enabled: Arc<AtomicBool>,
        fault: Option<Arc<AtomicU8>>,
    ) -> Result<Self, String> {
        let recording = fault.is_some();
        let props = properties! {
            "application.name" => "Raspiboard", "application.id" => APP_ID,
            "media.type" => "Audio", "media.category" => "Capture", "media.role" => "DSP",
            "node.autoconnect" => "false",
            "node.dont-fallback" => "true", "node.dont-reconnect" => "true",
            "audio.channels" => channels.to_string(),
        };
        let stream = pw::stream::StreamRc::new(
            core.clone(),
            if recording {
                "Raspiboard recording"
            } else {
                "Raspiboard passthrough capture"
            },
            props,
        )
        .map_err(|e| e.to_string())?;
        let error = Rc::new(RefCell::new(None));
        let on_error = error.clone();
        let listener = stream
            .add_local_listener_with_user_data(CaptureData {
                samples,
                enabled: enabled.clone(),
                channels,
                fault,
            })
            .state_changed(move |_, _, _, state| match state {
                pw::stream::StreamState::Error(message) => {
                    *on_error.borrow_mut() = Some(message.to_owned())
                }
                pw::stream::StreamState::Unconnected => {
                    *on_error.borrow_mut() = Some("The audio capture disconnected.".into())
                }
                _ => {}
            })
            .process(|stream, capture| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let Some(data) = buffer.datas_mut().first_mut() else {
                    return;
                };
                let offset = data.chunk().offset() as usize;
                let size = (data.chunk().size() as usize).min(data.as_raw().maxsize as usize);
                let stride = data.chunk().stride();
                let flags = data.chunk().flags().bits();
                capture.push(
                    data.data().as_deref().unwrap_or(&[]),
                    offset,
                    size,
                    stride,
                    flags,
                );
            })
            .register()
            .map_err(|e| e.to_string())?;
        let bytes = audio_format(RATE, channels as u32)?;
        let pod = spa::pod::Pod::from_bytes(&bytes).ok_or("Invalid capture format.")?;
        let flags = pw::stream::StreamFlags::MAP_BUFFERS;
        stream
            .connect(spa::utils::Direction::Input, None, flags, &mut [pod])
            .map_err(|e| format!("Cannot capture audio: {e}"))?;
        Ok(Self {
            links: None,
            _listener: listener,
            stream,
            error,
            source,
        })
    }

    pub fn prepare(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<bool, String> {
        if let Some(error) = self.error.borrow_mut().take() {
            return Err(error);
        }
        let id = self.stream.node_id();
        if id == pw::constants::ID_ANY {
            return Ok(false);
        }
        self.links
            .get_or_insert_with(|| Links::new(self.source, id))
            .prepare_capture(core, graph)
    }
}

pub(super) fn queue_capture(
    bytes: &[u8],
    offset: usize,
    size: usize,
    stride: usize,
    channels: usize,
    samples: &mut rtrb::Producer<[f32; 2]>,
) -> bool {
    if !(1..=2).contains(&channels) {
        return false;
    }
    let width = channels * 4;
    let stride = if stride == 0 { width } else { stride };
    if stride < width || bytes.len() < width {
        return false;
    }
    let size = size.min(bytes.len());
    if size < width {
        return false;
    }
    let mut position = offset % bytes.len();
    for _ in 0..=(size - width) / stride {
        let left = sample_at(bytes, position);
        let right = if channels == 1 {
            left
        } else {
            sample_at(bytes, (position + 4) % bytes.len())
        };
        if samples.push([left, right]).is_err() {
            return false;
        }
        position = (position + stride % bytes.len()) % bytes.len();
    }
    true
}

fn sample_at(bytes: &[u8], position: usize) -> f32 {
    let sample = if position + 4 <= bytes.len() {
        bytes[position..position + 4].try_into().unwrap()
    } else {
        std::array::from_fn(|index| bytes[(position + index) % bytes.len()])
    };
    f32::from_le_bytes(sample)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_capture_preserves_silence_and_reports_queue_overflow() {
        let (samples, mut output) = rtrb::RingBuffer::new(2);
        let fault = Arc::new(AtomicU8::new(0));
        let mut feed = CaptureData {
            samples,
            enabled: Arc::new(AtomicBool::new(true)),
            channels: 2,
            fault: Some(fault.clone()),
        };
        feed.push(&[255; 16], 0, 16, 8, EMPTY_CHUNK_FLAG);
        assert_eq!(output.pop().unwrap(), [0.0; 2]);
        assert_eq!(output.pop().unwrap(), [0.0; 2]);
        feed.push(&[0; 24], 0, 24, 8, 0);
        assert_eq!(fault.load(Ordering::Acquire), QUEUE_OVERFLOW);
    }

    #[test]
    fn recording_capture_rejects_corrupt_audio_without_enqueuing_it() {
        let (samples, mut output) = rtrb::RingBuffer::new(2);
        let fault = Arc::new(AtomicU8::new(0));
        let mut feed = CaptureData {
            samples,
            enabled: Arc::new(AtomicBool::new(true)),
            channels: 2,
            fault: Some(fault.clone()),
        };
        feed.push(
            &[0; 16],
            0,
            16,
            8,
            spa::buffer::ChunkFlags::CORRUPTED.bits(),
        );
        assert_eq!(fault.load(Ordering::Acquire), BAD_CAPTURE);
        assert!(output.pop().is_err());
    }
}
