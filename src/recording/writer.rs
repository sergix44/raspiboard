use super::{RecordingFormat, encoder::Encoder};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) const QUEUE_OVERFLOW: u8 = 1;
pub(crate) const BAD_CAPTURE: u8 = 2;
pub(crate) const QUEUE_FRAMES: usize = 65_536;

pub(crate) struct WriterJob {
    pub ready: mpsc::Receiver<Result<PathBuf, String>>,
    pub result: mpsc::Receiver<Result<Option<PathBuf>, String>>,
    pub fault: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WriterJob {
    pub fn start(
        directory: PathBuf,
        format: RecordingFormat,
    ) -> Result<(Self, rtrb::Producer<[f32; 2]>), String> {
        let (producer, mut samples) = rtrb::RingBuffer::<[f32; 2]>::new(QUEUE_FRAMES);
        let (ready_tx, ready) = mpsc::sync_channel(1);
        let (result_tx, result) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let fault = Arc::new(AtomicU8::new(0));
        let stopped = stop.clone();
        let faulted = fault.clone();
        let worker = thread::Builder::new()
            .name("recording-writer".into())
            .spawn(move || {
                let prepared = (|| {
                    std::fs::create_dir_all(&directory)
                        .map_err(|e| format!("{}: {e}", directory.display()))?;
                    let path = directory.join(format!(
                        "recording-{}.{}",
                        uuid::Uuid::now_v7(),
                        format.extension()
                    ));
                    Encoder::create(&path, format).map(|encoder| (path, encoder))
                })();
                let (path, mut encoder) = match prepared {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.clone()));
                        let _ = result_tx.send(Err(error));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(path.clone()));
                let mut written = 0_u64;
                let mut checkpoint = Instant::now();
                let mut error = None;
                let mut block = Vec::with_capacity(4096);
                loop {
                    // Observe Stop before reading: all preceding producer writes must drain.
                    let finishing =
                        stopped.load(Ordering::Acquire) || faulted.load(Ordering::Acquire) != 0;
                    block.clear();
                    while block.len() < block.capacity() {
                        let Ok(frame) = samples.pop() else {
                            break;
                        };
                        block.push(frame.map(|value| {
                            if value.is_finite() {
                                value.clamp(-1.0, 1.0 - f32::EPSILON)
                            } else {
                                0.0
                            }
                        }));
                    }
                    if !block.is_empty() {
                        if let Err(message) = encoder.write(&block) {
                            error = Some(message);
                            break;
                        }
                        written += block.len() as u64;
                    } else if finishing {
                        break;
                    } else {
                        thread::sleep(Duration::from_millis(2));
                    }
                    if checkpoint.elapsed() >= Duration::from_secs(1) {
                        if let Err(message) = encoder.checkpoint() {
                            error = Some(message);
                            break;
                        }
                        checkpoint = Instant::now();
                    }
                }
                if let Err(message) = encoder.finish() {
                    error = Some(message);
                }
                let fault = faulted.load(Ordering::Acquire);
                if error.is_none() {
                    error = match fault {
                        QUEUE_OVERFLOW => Some(
                            "Recording stopped because the disk encoder could not keep up.".into(),
                        ),
                        BAD_CAPTURE => {
                            Some("Recording stopped because the captured audio was invalid.".into())
                        }
                        _ => None,
                    };
                }
                let result = if let Some(error) = error {
                    Err(format!("{error} Captured file: {}", path.display()))
                } else if written == 0 {
                    std::fs::remove_file(&path).map(|_| None).map_err(|e| {
                        format!("Cannot remove empty recording {}: {e}", path.display())
                    })
                } else {
                    Ok(Some(path))
                };
                let _ = result_tx.send(result);
            })
            .map_err(|e| format!("Cannot start recording writer: {e}"))?;
        Ok((
            Self {
                ready,
                result,
                fault,
                stop,
                worker: Some(worker),
            },
            producer,
        ))
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl Drop for WriterJob {
    fn drop(&mut self) {
        self.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
