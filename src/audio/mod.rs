mod backend;
mod buffer;
mod decode;
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
    pub playback: PlaybackStatus,
    pub playbacks: Vec<PlaybackInstance>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub enum Command {
    Select(Option<Target>),
    SelectSource(Option<Target>),
    SetPassthrough(bool),
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
