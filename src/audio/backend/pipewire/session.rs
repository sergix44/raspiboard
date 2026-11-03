use super::{
    OutputVolumes, PlaybackInstance, PlaybackOptions, PlaybackStatus, Target, graph::Graph,
    job::DecodeJob, player::Player, route::Route,
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

struct RemotePlayback {
    player: Player,
    route: Route,
}

pub(super) struct Playback {
    remote: Option<RemotePlayback>,
    local: Player,
    active: bool,
}

impl Playback {
    fn players(&self) -> impl Iterator<Item = &Player> {
        self.remote
            .iter()
            .map(|remote| &remote.player)
            .chain(std::iter::once(&self.local))
    }

    fn players_mut(&mut self) -> impl Iterator<Item = &mut Player> {
        self.remote
            .iter_mut()
            .map(|remote| &mut remote.player)
            .chain(std::iter::once(&mut self.local))
    }

    fn drained(&self) -> bool {
        self.players()
            .all(|player| player.drained.load(Ordering::Acquire))
    }

    fn drain(&mut self) -> Result<bool, String> {
        for player in self.players_mut() {
            if player.finished.load(Ordering::Acquire) && !player.draining {
                player.stream.flush(true).map_err(|e| e.to_string())?;
                player.draining = true;
            }
        }
        Ok(self.drained())
    }

    pub fn retire(&mut self, since: Instant) -> bool {
        since.elapsed() < Duration::from_secs(3) && matches!(self.drain(), Ok(false))
    }
}

pub(super) struct Session {
    button_id: String,
    pub preview: bool,
    pub job: DecodeJob,
    pub paused: Arc<AtomicBool>,
    playback: Option<Playback>,
    pub started: Option<Instant>,
    drain_started: Option<Instant>,
    target: Option<Target>,
    sequence: u64,
}

impl Session {
    pub fn new(
        button_id: String,
        preview: bool,
        path: PathBuf,
        target: Option<Target>,
        options: PlaybackOptions,
        sequence: u64,
    ) -> Result<Self, String> {
        let job = DecodeJob::start(path, target.is_some(), options)?;
        Ok(Self {
            button_id,
            preview,
            job,
            paused: Arc::new(AtomicBool::new(false)),
            playback: None,
            started: None,
            drain_started: None,
            target,
            sequence,
        })
    }

    pub fn snapshot(&self) -> PlaybackInstance {
        PlaybackInstance {
            button_id: self.button_id.clone(),
            status: if self.paused.load(Ordering::Acquire) {
                PlaybackStatus::Paused
            } else if self
                .playback
                .as_ref()
                .is_some_and(|playback| playback.active)
            {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Loading
            },
            preview: self.preview,
        }
    }

    pub fn set_paused(&self, paused: bool) {
        // Finishing outputs must drain, even if other instances can still pause.
        if paused
            && self.playback.as_ref().is_some_and(|playback| {
                playback
                    .players()
                    .any(|player| player.draining || player.finished.load(Ordering::Acquire))
            })
        {
            return;
        }
        self.paused.store(paused, Ordering::Release);
    }

    pub fn stop(&mut self) -> Option<Playback> {
        let playback = self.playback.take()?;
        if !playback.active || playback.drained() {
            return None;
        }
        for player in playback.players() {
            player.stop.store(true, Ordering::Release);
        }
        Some(playback)
    }

    pub fn tick(
        &mut self,
        core: &pw::core::CoreRc,
        graph: &Graph,
        volumes: &OutputVolumes,
    ) -> Result<bool, String> {
        if self.playback.is_none() {
            match self.job.ready.try_recv() {
                Ok(Ok(decoded)) => {
                    let remote = if let Some(decoded) = decoded.remote {
                        let target = self.target.clone().ok_or("Select an application first.")?;
                        let route = Route::new(core, graph, target, self.sequence)?;
                        let player = Player::new(
                            core,
                            Some(&route.sink.name),
                            decoded,
                            volumes.remote.clone(),
                            self.paused.clone(),
                        )?;
                        Some(RemotePlayback { player, route })
                    } else {
                        None
                    };
                    let local = Player::new(
                        core,
                        None,
                        decoded.local,
                        volumes.local.clone(),
                        self.paused.clone(),
                    )?;
                    self.playback = Some(Playback {
                        remote,
                        local,
                        active: false,
                    });
                    self.started = Some(Instant::now());
                }
                Ok(Err(error)) => return Err(error),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("The audio decoder stopped unexpectedly.".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(playback) = &mut self.playback else {
            return Ok(false);
        };
        if !playback.active
            && self
                .started
                .is_some_and(|start| start.elapsed() > Duration::from_secs(10))
        {
            return Err("Audio routing timed out. Check that the selected app is recording and a desktop output is available.".into());
        }
        for player in playback.players() {
            if let Ok(Err(error)) = player.result.try_recv() {
                return Err(error);
            }
            if let Some(error) = player.error.borrow_mut().take() {
                return Err(error);
            }
        }
        let remote_ready = if let Some(remote) = &mut playback.remote {
            let output_ready = remote.route.prepare(core, graph)?;
            let input_ready = match remote.route.sink.id.get() {
                Some(sink) => graph.has_link(remote.player.stream.node_id(), sink)?,
                None => false,
            };
            output_ready && input_ready
        } else {
            true
        };
        let local_ready = playback.local.stream.state() == pw::stream::StreamState::Streaming;
        if playback.active
            && (playback
                .remote
                .as_ref()
                .is_some_and(|remote| !remote_ready && !remote.player.draining)
                || (!local_ready && !playback.local.draining))
        {
            return Err("The audio connection was removed. Play again to reconnect.".into());
        }
        if !playback.active && remote_ready && local_ready {
            for player in playback.players() {
                player.enabled.store(true, Ordering::Release);
            }
            playback.active = true;
        }
        if playback
            .players()
            .any(|player| player.finished.load(Ordering::Acquire))
        {
            self.drain_started.get_or_insert_with(Instant::now);
        }
        if playback.drain()? {
            return Ok(true);
        }
        if self
            .drain_started
            .is_some_and(|start| start.elapsed() > Duration::from_secs(3))
        {
            return Err("An output stopped consuming audio before playback finished.".into());
        }
        Ok(false)
    }
}
