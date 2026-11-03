use super::{capture::Capture, graph::Graph, recording_mix::RecordingMix};
use crate::{
    audio::RecordingStatus,
    recording::{RecordingFormat, writer::WriterJob},
};
use pipewire as pw;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub(super) type Completion = Result<Option<PathBuf>, String>;

pub(super) struct Recording {
    capture: Option<Capture>,
    mixer: Option<RecordingMix>,
    enabled: Arc<AtomicBool>,
    producer: Option<rtrb::Producer<[f32; 2]>>,
    job: WriterJob,
    started: Instant,
    ready: bool,
    active: bool,
    stopping: bool,
    error: Option<String>,
}

impl Recording {
    pub fn new(directory: PathBuf, format: RecordingFormat) -> Result<Self, String> {
        let (job, producer) = WriterJob::start(directory, format)?;
        Ok(Self {
            capture: None,
            mixer: None,
            enabled: Arc::new(AtomicBool::new(false)),
            producer: Some(producer),
            job,
            started: Instant::now(),
            ready: false,
            active: false,
            stopping: false,
            error: None,
        })
    }

    pub fn stop(&mut self) {
        self.stopping = true;
        self.capture = None;
        self.mixer = None;
        self.producer = None;
        self.job.stop();
    }

    pub fn finish(mut self) -> Completion {
        self.stop();
        let result = self
            .job
            .result
            .recv()
            .unwrap_or_else(|_| Err("The recording writer stopped unexpectedly.".into()));
        self.outcome(result)
    }

    fn outcome(&mut self, result: Completion) -> Completion {
        match (self.error.take(), result) {
            (Some(error), Ok(path)) => Err(match path {
                Some(path) => format!("{error} Captured file: {}", path.display()),
                None => error,
            }),
            (_, result) => result,
        }
    }

    pub fn poll(
        &mut self,
        core: &pw::core::CoreRc,
        graph: &Graph,
    ) -> (RecordingStatus, Option<Completion>) {
        if !self.stopping {
            let result = self.prepare(core, graph);
            if let Err(error) = result {
                self.error = Some(error);
                self.stop();
            }
        }
        let completion = match self.job.result.try_recv() {
            Ok(result) => Some(self.outcome(result)),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("The recording writer stopped unexpectedly.".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        };
        let status = if completion.is_some() {
            RecordingStatus::Idle
        } else if self.stopping {
            RecordingStatus::Saving
        } else if self.active {
            RecordingStatus::Recording
        } else {
            RecordingStatus::Starting
        };
        (status, completion)
    }

    fn prepare(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<(), String> {
        if self.job.fault.load(Ordering::Acquire) != 0 {
            self.stop();
            return Ok(());
        }
        if !self.ready {
            match self.job.ready.try_recv() {
                Ok(Ok(_)) => {
                    self.ready = true;
                }
                Ok(Err(error)) => return Err(error),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("The recording writer stopped.".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut mixer_ready = false;
        if self.ready {
            if self.mixer.is_none() {
                self.mixer = RecordingMix::new(core, graph)?;
            }
            if let Some(mixer) = &mut self.mixer {
                mixer_ready = mixer.prepare(core, graph)?;
                if self.capture.is_none()
                    && let Some(id) = mixer.sink.id.get()
                {
                    let producer = self.producer.take().ok_or("Recording was cancelled.")?;
                    self.capture = Some(Capture::recording_mix(
                        core,
                        id,
                        producer,
                        self.enabled.clone(),
                        self.job.fault.clone(),
                    )?);
                }
            }
        }
        if let Some(capture) = &mut self.capture {
            let ready = capture.prepare(core, graph)? && mixer_ready;
            if self.active && !ready {
                return Err("The recording output disconnected.".into());
            }
            self.active = ready;
            self.enabled.store(ready, Ordering::Release);
        }
        if !self.active && self.started.elapsed() > Duration::from_secs(10) {
            return Err("Recording could not connect to the default output and microphone.".into());
        }
        Ok(())
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{audio::RecordingStatus, recording::RecordingFormat};

    #[test]
    fn recording_stop_is_independent_and_finish_waits_for_the_writer() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/recording/engine");
        std::fs::create_dir_all(&root).unwrap();
        let directory = tempfile::tempdir_in(root).unwrap();
        let mut engine = Engine::default();
        let volumes = OutputVolumes::default();
        engine.command(
            Command::StartRecording {
                directory: directory.path().into(),
                format: RecordingFormat::Flac,
            },
            &volumes,
        );
        assert_eq!(engine.snapshot.recording, RecordingStatus::Starting);
        engine.command(Command::Stop, &volumes);
        assert_eq!(engine.snapshot.recording, RecordingStatus::Starting);
        engine.command(Command::SelectSource(None), &volumes);
        assert_eq!(engine.snapshot.recording, RecordingStatus::Starting);
        engine.snapshot.passthrough = PassthroughStatus::Active;
        let (done, result) = async_channel::bounded(1);
        engine.command(Command::FinishRecording(done), &volumes);
        assert_eq!(engine.snapshot.recording, RecordingStatus::Saving);
        assert!(result.try_recv().is_err());
        assert_eq!(engine.snapshot.passthrough, PassthroughStatus::Active);
        assert!(engine.recording.is_some());
        let completion = engine.recording.take().unwrap().finish();
        engine.recording_completed(completion);
        assert_eq!(result.recv_blocking().unwrap(), Ok(()));
        assert_eq!(engine.snapshot.recording, RecordingStatus::Idle);
    }
}
