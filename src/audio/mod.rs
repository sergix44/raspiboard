mod backend;
mod buffer;
pub(crate) mod decode;
mod job;
mod playback_options;
pub mod waveform;

use iced::futures::{SinkExt, Stream};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

mod target;
pub use playback_options::PlaybackOptions;
pub use target::{Target, TargetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackStatus {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PassthroughStatus {
    #[default]
    Off,
    Connecting,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingStatus {
    #[default]
    Idle,
    Starting,
    Recording,
    Saving,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackInstance {
    pub button_id: String,
    pub status: PlaybackStatus,
    pub preview: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub connected: bool,
    pub targets: Vec<Target>,
    pub selected: Option<Target>,
    pub sources: Vec<Target>,
    pub source: Option<Target>,
    pub passthrough: PassthroughStatus,
    pub recording: RecordingStatus,
    pub last_recording: Option<PathBuf>,
    pub playback: PlaybackStatus,
    pub playbacks: Vec<PlaybackInstance>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum Command {
    Select(Option<Target>),
    SelectSource(Option<Target>),
    SetPassthrough(bool),
    StartRecording {
        directory: PathBuf,
        format: crate::recording::RecordingFormat,
    },
    StopRecording,
    FinishRecording(async_channel::Sender<Result<(), String>>),
    Play {
        button_id: String,
        overlap: bool,
        path: PathBuf,
        target: Option<Target>,
        options: PlaybackOptions,
    },
    Preview {
        button_id: String,
        path: PathBuf,
        options: PlaybackOptions,
    },
    Stop,
    Pause,
    Resume,
    SetVolumes {
        remote: u8,
        local: u8,
    },
}

#[derive(Debug, Clone)]
pub struct Controller(mpsc::SyncSender<Command>);

impl Controller {
    pub fn send(&self, command: Command) -> Result<(), String> {
        self.0.try_send(command).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => "Audio controls are busy. Please try again.".into(),
            mpsc::TrySendError::Disconnected(_) => "The audio worker is not available.".into(),
        })
    }

    pub async fn finish_recording(self) -> Result<(), String> {
        let (done, receiver) = async_channel::bounded(1);
        let (sent, status) = async_channel::bounded(1);
        thread::Builder::new()
            .name("recording-finish".into())
            .spawn(move || {
                let result = self.0.send(Command::FinishRecording(done)).map_err(|_| {
                    "The audio worker stopped before recording could finish.".to_owned()
                });
                let _ = sent.send_blocking(result);
            })
            .map_err(|e| e.to_string())?;
        status.recv().await.map_err(|e| e.to_string())??;
        receiver
            .recv()
            .await
            .map_err(|_| "The audio worker stopped before recording could finish.".to_owned())?
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Ready(Controller),
    Changed(Box<Snapshot>),
}

struct WorkerGuard(Arc<AtomicBool>);

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub fn events() -> impl Stream<Item = Event> {
    iced::stream::channel(
        8,
        async move |mut output: iced::futures::channel::mpsc::Sender<Event>| {
            let (commands, receiver) = mpsc::sync_channel(32);
            let (snapshots, changes) = async_channel::bounded(1);
            let shutdown = Arc::new(AtomicBool::new(false));
            let _guard = WorkerGuard(shutdown.clone());
            let worker = thread::Builder::new()
                .name("audio".into())
                .spawn(move || backend::run(receiver, snapshots, shutdown));
            if let Err(error) = worker {
                let _ = output
                    .send(Event::Changed(Box::new(Snapshot {
                        error: Some(format!("Cannot start audio: {error}")),
                        ..Default::default()
                    })))
                    .await;
                return;
            }
            if output
                .send(Event::Ready(Controller(commands)))
                .await
                .is_err()
            {
                return;
            }
            while let Ok(snapshot) = changes.recv().await {
                if output
                    .send(Event::Changed(Box::new(snapshot)))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            let _ = output
                .send(Event::Changed(Box::new(Snapshot {
                    error: Some("The audio worker stopped. Restart the app to reconnect.".into()),
                    ..Default::default()
                })))
                .await;
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_finish_waits_for_acknowledgment_after_preceding_commands() {
        let (sender, receiver) = mpsc::sync_channel(32);
        let controller = Controller(sender);
        controller.send(Command::StopRecording).unwrap();
        let finish =
            thread::spawn(move || iced::futures::executor::block_on(controller.finish_recording()));
        assert!(matches!(receiver.recv().unwrap(), Command::StopRecording));
        let Command::FinishRecording(done) = receiver.recv().unwrap() else {
            panic!("finish command must follow Stop");
        };
        assert!(!finish.is_finished());
        done.send_blocking(Ok(())).unwrap();
        assert_eq!(finish.join().unwrap(), Ok(()));
    }

    #[test]
    fn recording_finish_reports_a_disconnected_worker_instead_of_waiting_forever() {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        let result = iced::futures::executor::block_on(Controller(sender).finish_recording());
        assert!(result.unwrap_err().contains("audio worker stopped"));
    }
}
