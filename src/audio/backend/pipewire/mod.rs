use self::{
    discovery::Discovery,
    graph::Graph,
    session::{Playback, Session},
};
mod capture;
mod discovery;
mod graph;
mod passthrough;
mod player;
mod recording;
mod recording_mix;
mod route;
mod session;
use crate::audio::{
    Command, PassthroughStatus, PlaybackInstance, PlaybackStatus, RecordingStatus, Snapshot,
};
use crate::audio::{PlaybackOptions, Target, buffer, job};
use pipewire as pw;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct OutputVolumes {
    pub remote: Arc<AtomicU8>,
    pub local: Arc<AtomicU8>,
}

impl Default for OutputVolumes {
    fn default() -> Self {
        Self {
            remote: Arc::new(AtomicU8::new(100)),
            local: Arc::new(AtomicU8::new(100)),
        }
    }
}

impl OutputVolumes {
    fn set(&self, remote: u8, local: u8) {
        self.remote.store(remote.min(100), Ordering::Relaxed);
        self.local.store(local.min(100), Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Engine {
    snapshot: Snapshot,
    target_initialized: bool,
    sessions: Vec<Session>,
    retiring: Vec<(Playback, Instant)>,
    sequence: u64,
    passthrough: Option<passthrough::Passthrough>,
    retiring_passthrough: Vec<passthrough::RetiringPassthrough>,
    recording: Option<recording::Recording>,
    recording_finish: Vec<async_channel::Sender<Result<(), String>>>,
}

fn playback_status(instances: &[PlaybackInstance]) -> PlaybackStatus {
    [
        PlaybackStatus::Playing,
        PlaybackStatus::Paused,
        PlaybackStatus::Loading,
    ]
    .into_iter()
    .find(|status| instances.iter().any(|instance| instance.status == *status))
    .unwrap_or(PlaybackStatus::Idle)
}

impl Engine {
    fn refresh_targets(&mut self, graph: &Graph) {
        self.snapshot.targets = graph.targets();
        self.snapshot.sources = graph.sources();
        if let Some(source) = &self.snapshot.source {
            self.snapshot.source = self
                .snapshot
                .sources
                .iter()
                .find(|current| current.id == source.id)
                .cloned();
            if self.snapshot.source.is_none() {
                let sharing = self.snapshot.passthrough != PassthroughStatus::Off;
                self.stop_passthrough();
                if sharing {
                    self.snapshot.error =
                        Some("The passthrough source closed. Select an application again.".into());
                }
            }
        }
        if let Some(selected) = &self.snapshot.selected {
            self.target_initialized = true;
            self.snapshot.selected = self
                .snapshot
                .targets
                .iter()
                .find(|target| target.id == selected.id)
                .cloned();
            if self.snapshot.selected.is_none() {
                let sharing = self.snapshot.passthrough != PassthroughStatus::Off;
                self.stop_passthrough();
                let error =
                    "The selected recording stream closed. Select an application again.".into();
                if !self.sessions.iter().any(|session| session.preview) {
                    self.fail(error);
                } else if sharing {
                    self.snapshot.error = Some(error);
                }
            }
        }
        if !self.target_initialized
            && let Some(target) = self.snapshot.targets.first()
        {
            self.snapshot.selected = Some(target.clone());
            self.target_initialized = true;
        }
    }

    fn stop_passthrough(&mut self) {
        if let Some(retiring) = self
            .passthrough
            .take()
            .and_then(passthrough::Passthrough::stop)
        {
            self.retiring_passthrough.push(retiring);
        }
        self.snapshot.passthrough = PassthroughStatus::Off;
    }

    fn refresh_playback(&mut self) {
        self.snapshot.playbacks = self.sessions.iter().map(Session::snapshot).collect();
        self.snapshot.playback = playback_status(&self.snapshot.playbacks);
    }

    fn stop(&mut self) {
        for mut session in self.sessions.drain(..) {
            if let Some(playback) = session.stop() {
                self.retiring.push((playback, Instant::now()));
            }
        }
        self.refresh_playback();
    }

    fn finish_session(&mut self, index: usize, error: Option<String>) {
        let mut session = self.sessions.remove(index);
        if let Some(playback) = session.stop() {
            self.retiring.push((playback, Instant::now()));
        }
        if let Some(error) = error {
            self.snapshot.error = Some(error);
        }
        self.refresh_playback();
    }

    fn fail(&mut self, error: String) {
        self.stop();
        self.snapshot.error = Some(error);
    }

    fn set_paused(&mut self, paused: bool) {
        if self.sessions.is_empty() || (paused && self.snapshot.playback != PlaybackStatus::Playing)
        {
            return;
        }
        for session in &self.sessions {
            session.set_paused(paused);
        }
        self.refresh_playback();
    }

    fn start_decode(
        &mut self,
        button_id: String,
        preview: bool,
        overlap: bool,
        path: std::path::PathBuf,
        target: Option<Target>,
        options: PlaybackOptions,
    ) {
        self.sequence += 1;
        match Session::new(
            button_id,
            preview,
            overlap,
            path,
            target,
            options,
            self.sequence,
        ) {
            Ok(session) => {
                self.target_initialized = true;
                self.sessions.push(session);
            }
            Err(error) => self.snapshot.error = Some(error),
        }
        self.refresh_playback();
    }

    fn command(&mut self, command: Command, volumes: &OutputVolumes) {
        match command {
            Command::StartRecording { directory, format } => {
                if self.recording.is_none() && self.recording_finish.is_empty() {
                    self.snapshot.error = None;
                    self.snapshot.last_recording = None;
                    match recording::Recording::new(directory, format) {
                        Ok(recording) => {
                            self.recording = Some(recording);
                            self.snapshot.recording = RecordingStatus::Starting;
                        }
                        Err(error) => self.snapshot.error = Some(error),
                    }
                }
            }
            Command::StopRecording => self.stop_recording(),
            Command::FinishRecording(done) => {
                if self.recording.is_some() {
                    self.recording_finish.push(done);
                    self.stop_recording();
                } else {
                    let _ = done.try_send(Ok(()));
                }
            }
            Command::SelectSource(source) => {
                self.stop_passthrough();
                self.snapshot.error = None;
                self.snapshot.source = source.and_then(|source| {
                    self.snapshot
                        .sources
                        .iter()
                        .find(|current| current.id == source.id)
                        .cloned()
                });
            }
            Command::SetPassthrough(enabled) => {
                self.stop_passthrough();
                self.snapshot.error = None;
                if enabled {
                    if self.snapshot.source.is_some() && self.snapshot.selected.is_some() {
                        self.snapshot.passthrough = PassthroughStatus::Connecting;
                    } else {
                        self.snapshot.error = Some("Select a source and an output application before enabling passthrough.".into());
                    }
                }
            }
            Command::SetVolumes { remote, local } => volumes.set(remote, local),
            Command::Pause => self.set_paused(true),
            Command::Resume => self.set_paused(false),
            Command::Seek {
                playback_id,
                position_ms,
            } => {
                if self.sessions.len() == 1 {
                    self.sequence += 1;
                    match self.sessions[0].seek(playback_id, position_ms, self.sequence) {
                        Ok(Some(playback)) => self.retiring.push((playback, Instant::now())),
                        Ok(None) => {}
                        Err(error) => self.snapshot.error = Some(error),
                    }
                    self.refresh_playback();
                }
            }
            Command::Preview {
                button_id,
                path,
                options,
            } => {
                self.stop();
                self.snapshot.error = None;
                self.start_decode(button_id, true, false, path, None, options);
            }
            Command::Stop => {
                self.stop();
                self.snapshot.error = None;
            }
            Command::Select(target) => {
                self.stop_passthrough();
                self.target_initialized = true;
                self.stop();
                self.snapshot.error = None;
                self.snapshot.selected = target.and_then(|target| {
                    self.snapshot
                        .targets
                        .iter()
                        .find(|current| current.id == target.id)
                        .cloned()
                });
            }
            Command::Play {
                button_id,
                overlap,
                path,
                target,
                options,
            } => {
                if !overlap || self.sessions.iter().any(|session| session.preview) {
                    self.stop();
                }
                self.snapshot.error = None;
                let matches_selection = match (target.as_ref(), self.snapshot.selected.as_ref()) {
                    (Some(requested), Some(current)) => current.id == requested.id,
                    (None, None) => true,
                    _ => false,
                };
                if !matches_selection {
                    self.fail(
                        "The output application changed. Play again to use the current selection."
                            .into(),
                    );
                    return;
                }
                self.set_paused(false);
                self.start_decode(button_id, false, overlap, path, target, options);
            }
        }
    }

    fn tick(&mut self, core: &pw::core::CoreRc, graph: &Graph, volumes: &OutputVolumes) {
        if let Some(recording) = &mut self.recording {
            let (status, completion) = recording.poll(core, graph);
            self.snapshot.recording = status;
            if let Some(completion) = completion {
                self.recording_completed(completion);
            }
        }
        self.retiring_passthrough
            .retain_mut(passthrough::RetiringPassthrough::retain);
        self.retiring
            .retain_mut(|(playback, since)| playback.retire(*since));
        self.refresh_targets(graph);
        if self.snapshot.passthrough != PassthroughStatus::Off {
            let result = self.tick_passthrough(core, graph, volumes);
            match result {
                Ok(true) => self.snapshot.passthrough = PassthroughStatus::Active,
                Ok(false) => {}
                Err(error) => {
                    self.stop_passthrough();
                    self.snapshot.error = Some(format!("Passthrough: {error}"));
                }
            }
        }
        let mut index = 0;
        while index < self.sessions.len() {
            match self.sessions[index].tick(core, graph, volumes) {
                Ok(true) => self.finish_session(index, None),
                Err(error) => self.finish_session(index, Some(error)),
                Ok(false) => index += 1,
            }
        }
        self.refresh_playback();
    }

    fn tick_passthrough(
        &mut self,
        core: &pw::core::CoreRc,
        graph: &Graph,
        volumes: &OutputVolumes,
    ) -> Result<bool, String> {
        if self.passthrough.is_none() {
            self.sequence += 1;
            let source = self
                .snapshot
                .source
                .as_ref()
                .ok_or("Select a source application.")?;
            let target = self
                .snapshot
                .selected
                .clone()
                .ok_or("Select an output application.")?;
            self.passthrough = Some(passthrough::Passthrough::new(
                core,
                graph,
                source,
                target,
                self.sequence,
                volumes,
            )?);
        }
        self.passthrough.as_mut().unwrap().tick(core, graph)
    }

    fn stop_recording(&mut self) {
        if let Some(recording) = &mut self.recording {
            recording.stop();
            self.snapshot.recording = RecordingStatus::Saving;
        }
    }

    fn recording_completed(&mut self, completion: recording::Completion) {
        self.recording = None;
        self.snapshot.recording = RecordingStatus::Idle;
        let acknowledgment = match completion {
            Ok(path) => {
                self.snapshot.last_recording = path;
                Ok(())
            }
            Err(error) => {
                self.snapshot.error = Some(error.clone());
                Err(error)
            }
        };
        for done in self.recording_finish.drain(..) {
            let _ = done.try_send(acknowledgment.clone());
        }
    }
}

pub fn run(
    commands: mpsc::Receiver<Command>,
    snapshots: async_channel::Sender<Snapshot>,
    shutdown: Arc<AtomicBool>,
) {
    pw::init();
    let commands = Rc::new(commands);
    let volumes = OutputVolumes::default();
    while !shutdown.load(Ordering::Acquire) {
        for command in commands.try_iter() {
            match command {
                Command::SetVolumes { remote, local } => volumes.set(remote, local),
                Command::FinishRecording(done) => {
                    let _ = done.try_send(Ok(()));
                }
                _ => {}
            }
        }
        let result = connection(
            commands.clone(),
            snapshots.clone(),
            shutdown.clone(),
            volumes.clone(),
        );
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let error = result
            .err()
            .unwrap_or_else(|| "PipeWire disconnected.".into());
        let snapshot = Snapshot {
            error: Some(format!("{error} Retrying…")),
            ..Default::default()
        };
        for _ in 0..10 {
            if shutdown.load(Ordering::Acquire) {
                return;
            }
            let _ = snapshots.try_send(snapshot.clone());
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn connection_error(id: u32, result: i32, message: &str) -> Option<String> {
    let resource_gone = result.checked_neg().is_some_and(|errno| {
        std::io::Error::from_raw_os_error(errno).kind() == std::io::ErrorKind::NotFound
    });
    (id == pw::core::PW_ID_CORE && !resource_gone).then(|| message.to_owned())
}

fn connection(
    commands: Rc<mpsc::Receiver<Command>>,
    snapshots: async_channel::Sender<Snapshot>,
    shutdown: Arc<AtomicBool>,
    volumes: OutputVolumes,
) -> Result<(), String> {
    let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(|e| e.to_string())?;
    let context = pw::context::ContextRc::new(&mainloop, None).map_err(|e| e.to_string())?;
    let core = context
        .connect_rc(None)
        .map_err(|e| format!("Cannot connect to PipeWire: {e}"))?;
    let errors = Rc::new(RefCell::new(None));
    let on_error = errors.clone();
    let weak_loop = mainloop.downgrade();
    let _core_listener = core
        .add_listener_local()
        .error(move |id, sequence, result, message| {
            let Some(error) = connection_error(id, result, message) else {
                // Removed resources can race with queued destroy requests on the core.
                log::debug!(target: "raspiboard::audio", "PipeWire object request: id={id} sequence={sequence} result={result}: {message}");
                return;
            };
            *on_error.borrow_mut() = Some(error);
            if let Some(mainloop) = weak_loop.upgrade() {
                mainloop.quit();
            }
        })
        .register();
    let graph = Rc::new(RefCell::new(Graph::default()));
    let _discovery = Discovery::new(&core, graph.clone())?;
    let engine = Rc::new(RefCell::new(Engine::default()));
    engine.borrow_mut().snapshot.connected = true;
    let tick_engine = engine.clone();
    let tick_errors = errors.clone();
    let weak_loop = mainloop.downgrade();
    let tick_core = core.clone();
    let last_snapshot = RefCell::new(None);
    let timer = mainloop.loop_().add_timer(move |_| {
        if shutdown.load(Ordering::Acquire) {
            if let Some(mainloop) = weak_loop.upgrade() {
                mainloop.quit();
            }
            return;
        }
        let mut engine = tick_engine.borrow_mut();
        let graph = graph.borrow();
        if let Some(error) = tick_errors.borrow_mut().take() {
            engine.stop_passthrough();
            engine.fail(error);
        }
        engine.tick(&tick_core, &graph, &volumes);
        for command in commands.try_iter().take(32) {
            engine.command(command, &volumes);
        }
        let mut last_snapshot = last_snapshot.borrow_mut();
        if last_snapshot.as_ref() != Some(&engine.snapshot)
            && snapshots.try_send(engine.snapshot.clone()).is_ok()
        {
            *last_snapshot = Some(engine.snapshot.clone());
        }
    });
    timer
        .update_timer(
            Some(Duration::from_millis(25)),
            Some(Duration::from_millis(25)),
        )
        .into_result()
        .map_err(|e| e.to_string())?;
    mainloop.run();
    engine.borrow_mut().stop();
    engine.borrow_mut().stop_passthrough();
    let recording = engine.borrow_mut().recording.take();
    let mut error = errors.borrow_mut().take();
    if let Some(recording) = recording {
        let completion = recording.finish();
        let detail = match &completion {
            Ok(Some(path)) => Some(format!("Captured recording: {}", path.display())),
            Err(message) => Some(message.clone()),
            Ok(None) => None,
        };
        if let Some(detail) = detail {
            error = Some(format!(
                "{} {detail}",
                error.unwrap_or_else(|| "PipeWire disconnected.".into())
            ));
        }
        engine.borrow_mut().recording_completed(completion);
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::graph::Kind;
    use super::*;

    #[test]
    fn passthrough_requires_both_endpoints_and_is_independent_of_clip_transport() {
        let mut engine = Engine::default();
        let volumes = OutputVolumes::default();
        engine.command(Command::SetPassthrough(true), &volumes);
        assert_eq!(
            engine.snapshot.passthrough,
            crate::audio::PassthroughStatus::Off
        );
        assert!(engine.snapshot.error.is_some());
        engine.snapshot.sources = vec![target(100)];
        engine.command(Command::SelectSource(Some(target(100))), &volumes);
        engine.snapshot.selected = Some(target(200));
        engine.command(Command::SetPassthrough(true), &volumes);
        assert_eq!(
            engine.snapshot.passthrough,
            crate::audio::PassthroughStatus::Connecting
        );
        for command in [Command::Stop, Command::Pause, Command::Resume] {
            engine.command(command, &volumes);
            assert_eq!(
                engine.snapshot.passthrough,
                crate::audio::PassthroughStatus::Connecting
            );
        }
        engine.command(Command::Select(None), &volumes);
        assert_eq!(
            engine.snapshot.passthrough,
            crate::audio::PassthroughStatus::Off
        );
    }

    #[test]
    fn passthrough_source_disappearance_and_reused_ids_disable_sharing() {
        let mut engine = Engine::default();
        engine.snapshot.source = Some(target(200));
        engine.snapshot.passthrough = crate::audio::PassthroughStatus::Active;
        let mut graph = Graph::default();
        graph.insert(
            20,
            Kind::Node,
            [
                ("object.serial".into(), "999".into()),
                ("media.class".into(), "Stream/Output/Audio".into()),
            ]
            .into(),
        );
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.source.is_none());
        assert_eq!(
            engine.snapshot.passthrough,
            crate::audio::PassthroughStatus::Off
        );
        assert!(engine.snapshot.error.is_some());
    }

    #[test]
    fn passthrough_is_not_stopped_by_a_rejected_clip_command() {
        let mut engine = Engine::default();
        engine.snapshot.passthrough = PassthroughStatus::Active;
        engine.command(
            Command::Play {
                button_id: "test".into(),
                overlap: false,
                path: "must-not-open.wav".into(),
                target: Some(target(200)),
                options: PlaybackOptions::default(),
            },
            &OutputVolumes::default(),
        );
        assert_eq!(engine.snapshot.passthrough, PassthroughStatus::Active);
        assert!(engine.snapshot.error.is_some());
    }

    #[test]
    fn passthrough_source_changes_do_not_stop_clips_and_stale_sources_are_rejected() {
        let mut engine = Engine::default();
        let volumes = OutputVolumes::default();
        engine.snapshot.sources = vec![target(100), target(200)];
        engine.snapshot.source = Some(target(100));
        engine.snapshot.passthrough = PassthroughStatus::Active;
        engine.snapshot.playback = PlaybackStatus::Playing;
        engine.command(Command::SelectSource(Some(target(200))), &volumes);
        assert_eq!(engine.snapshot.source, Some(target(200)));
        assert_eq!(engine.snapshot.passthrough, PassthroughStatus::Off);
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Playing);
        engine.command(Command::SelectSource(Some(target(999))), &volumes);
        assert!(engine.snapshot.source.is_none());
        engine.snapshot.selected = Some(target(300));
        engine.command(Command::SetPassthrough(true), &volumes);
        assert_eq!(engine.snapshot.passthrough, PassthroughStatus::Off);
    }

    fn target(serial: u64) -> Target {
        Target {
            id: format!("pipewire:20:{serial}").into(),
            display_label: "Recorder [20]".into(),
            label: "Recorder".into(),
        }
    }

    fn recording_app(graph: &mut Graph, id: u32) {
        graph.insert(
            id,
            Kind::Node,
            [
                ("object.serial".into(), (id as u64 * 10).to_string()),
                ("media.class".into(), "Stream/Input/Audio".into()),
            ]
            .into(),
        );
    }

    #[test]
    fn selected_application_closing_does_not_cancel_local_preview() {
        let mut engine = Engine::default();
        engine.snapshot.selected = Some(target(200));
        engine.command(
            Command::Preview {
                button_id: "test".into(),
                path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/audio/tone.flac"),
                options: super::PlaybackOptions::default(),
            },
            &OutputVolumes::default(),
        );
        engine.refresh_targets(&Graph::default());
        assert!(engine.snapshot.selected.is_none());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        assert!(!engine.sessions.is_empty());
        assert!(engine.snapshot.error.is_none());
    }

    #[test]
    fn passthrough_destination_closing_during_preview_reports_loss_without_stopping_preview() {
        let mut engine = Engine::default();
        engine.snapshot.selected = Some(target(200));
        engine.snapshot.passthrough = PassthroughStatus::Active;
        engine.command(
            Command::Preview {
                button_id: "test".into(),
                path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/audio/tone.flac"),
                options: PlaybackOptions::default(),
            },
            &OutputVolumes::default(),
        );
        assert_eq!(engine.snapshot.passthrough, PassthroughStatus::Active);
        engine.refresh_targets(&Graph::default());
        assert_eq!(engine.snapshot.passthrough, PassthroughStatus::Off);
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        assert!(
            engine
                .snapshot
                .error
                .as_ref()
                .is_some_and(|error| error.contains("recording stream closed"))
        );
    }

    #[test]
    fn preview_is_local_only_and_preserves_selected_application() {
        let mut engine = Engine::default();
        let selected = target(200);
        engine.snapshot.selected = Some(selected.clone());
        engine.command(
            Command::Preview {
                button_id: "test".into(),
                path: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/audio/tone.flac"),
                options: super::PlaybackOptions::default(),
            },
            &OutputVolumes::default(),
        );
        assert_eq!(engine.snapshot.selected, Some(selected));
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        let outputs = engine.sessions[0]
            .job
            .ready
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert!(outputs.remote.is_none());
    }

    #[test]
    fn pause_and_resume_do_not_start_idle_or_loading_playback() {
        for status in [PlaybackStatus::Idle, PlaybackStatus::Loading] {
            let mut engine = Engine::default();
            engine.snapshot.playback = status;
            engine.command(Command::Pause, &OutputVolumes::default());
            assert_eq!(engine.snapshot.playback, status);
            engine.command(Command::Resume, &OutputVolumes::default());
            assert_eq!(engine.snapshot.playback, status);
            assert!(engine.sessions.is_empty());
            engine.command(Command::Stop, &OutputVolumes::default());
            assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
        }
    }

    #[test]
    fn selects_first_available_application_without_starting_playback() {
        let mut engine = Engine::default();
        let mut graph = Graph::default();
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());

        recording_app(&mut graph, 30);
        recording_app(&mut graph, 20);
        engine.refresh_targets(&graph);
        assert_eq!(engine.snapshot.selected, graph.targets().first().cloned());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
        assert!(engine.sessions.is_empty());

        recording_app(&mut graph, 10);
        engine.refresh_targets(&graph);
        assert_eq!(
            engine.snapshot.selected.as_ref().unwrap().id,
            "pipewire:20:200".into()
        );
    }

    #[test]
    fn choosing_none_stops_playback_and_survives_discovery_updates() {
        let mut engine = Engine::default();
        let mut graph = Graph::default();
        recording_app(&mut graph, 20);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_some());
        engine.snapshot.playback = PlaybackStatus::Playing;

        engine.command(Command::Select(None), &OutputVolumes::default());
        recording_app(&mut graph, 10);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);

        graph.remove(10);
        graph.remove(20);
        engine.refresh_targets(&graph);
        recording_app(&mut graph, 30);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
    }

    #[test]
    fn choosing_none_before_discovery_prevents_default_selection() {
        let mut engine = Engine::default();
        engine.command(Command::Select(None), &OutputVolumes::default());
        let mut graph = Graph::default();
        recording_app(&mut graph, 20);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
    }

    #[test]
    fn explicit_selection_survives_new_applications_and_clears_when_closed() {
        let mut engine = Engine::default();
        let mut graph = Graph::default();
        recording_app(&mut graph, 20);
        recording_app(&mut graph, 30);
        engine.refresh_targets(&graph);
        let selected = engine.snapshot.targets[1].clone();
        engine.command(
            Command::Select(Some(selected.clone())),
            &OutputVolumes::default(),
        );
        recording_app(&mut graph, 10);
        engine.refresh_targets(&graph);
        assert_eq!(engine.snapshot.selected, Some(selected));

        engine.snapshot.playback = PlaybackStatus::Playing;
        graph.remove(30);
        engine.refresh_targets(&graph);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
        assert!(engine.snapshot.error.is_some());
    }

    #[test]
    fn clears_selection_and_playback_when_a_node_id_is_reused() {
        let mut graph = Graph::default();
        graph.insert(
            20,
            Kind::Node,
            [
                ("object.serial".into(), "999".into()),
                ("media.class".into(), "Stream/Input/Audio".into()),
            ]
            .into(),
        );
        let mut engine = Engine::default();
        engine.snapshot.selected = Some(target(200));
        engine.snapshot.playback = PlaybackStatus::Playing;
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
        assert!(engine.snapshot.error.is_some());
        assert_eq!(engine.snapshot.targets[0].id, "pipewire:20:999".into());
    }

    #[test]
    fn rejects_a_stale_play_command_after_destination_changes() {
        let mut engine = Engine::default();
        engine.snapshot.selected = Some(target(999));
        engine.command(
            Command::Play {
                button_id: "test".into(),
                overlap: false,
                path: "must-not-be-opened.wav".into(),
                options: Default::default(),
                target: Some(target(200)),
            },
            &OutputVolumes::default(),
        );
        assert!(engine.sessions.is_empty());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
        assert!(engine.snapshot.error.is_some());
    }

    #[test]
    fn crop_validation_does_not_start_the_routing_timeout() {
        let mut engine = Engine::default();
        engine.command(
            Command::Play {
                button_id: "test".into(),
                overlap: false,
                path: "missing.wav".into(),
                target: None,
                options: super::PlaybackOptions {
                    end_ms: Some(100_000),
                    ..Default::default()
                },
            },
            &OutputVolumes::default(),
        );
        assert!(!engine.sessions.is_empty());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        assert!(engine.sessions[0].started.is_none());
    }

    #[test]
    fn none_allows_a_local_decode_job_without_selecting_an_application() {
        let mut engine = Engine::default();
        engine.command(
            Command::Play {
                button_id: "test".into(),
                overlap: false,
                path: "missing.wav".into(),
                options: Default::default(),
                target: None,
            },
            &OutputVolumes::default(),
        );
        assert!(!engine.sessions.is_empty());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        assert!(engine.snapshot.selected.is_none());
        assert!(engine.snapshot.error.is_none());
    }

    #[test]
    fn local_playback_keeps_none_when_recording_applications_appear_and_close() {
        let mut engine = Engine::default();
        engine.command(
            Command::Play {
                button_id: "test".into(),
                overlap: false,
                path: "missing.wav".into(),
                options: Default::default(),
                target: None,
            },
            &OutputVolumes::default(),
        );
        let mut graph = Graph::default();
        recording_app(&mut graph, 20);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        graph.remove(20);
        engine.refresh_targets(&graph);
        assert!(engine.snapshot.selected.is_none());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
        assert!(engine.snapshot.error.is_none());
    }

    #[test]
    fn stale_local_or_remote_commands_cannot_change_the_current_destination() {
        for (selected, requested) in [(Some(target(200)), None), (None, Some(target(200)))] {
            let mut engine = Engine::default();
            engine.snapshot.selected = selected.clone();
            engine.command(
                Command::Play {
                    button_id: "test".into(),
                    overlap: false,
                    path: "must-not-be-opened.wav".into(),
                    options: Default::default(),
                    target: requested,
                },
                &OutputVolumes::default(),
            );
            assert!(engine.sessions.is_empty());
            assert_eq!(engine.snapshot.selected, selected);
            assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
            assert!(engine.snapshot.error.is_some());
        }
    }

    #[test]
    fn output_gains_are_independent_and_clamped() {
        let mut engine = Engine::default();
        let volumes = OutputVolumes::default();
        engine.command(
            Command::SetVolumes {
                remote: 25,
                local: 80,
            },
            &volumes,
        );
        assert_eq!(volumes.remote.load(Ordering::Relaxed), 25);
        assert_eq!(volumes.local.load(Ordering::Relaxed), 80);
        engine.command(
            Command::SetVolumes {
                remote: 255,
                local: 0,
            },
            &volumes,
        );
        assert_eq!(volumes.remote.load(Ordering::Relaxed), 100);
        assert_eq!(volumes.local.load(Ordering::Relaxed), 0);
        engine.command(Command::Stop, &volumes);
        assert_eq!(volumes.remote.load(Ordering::Relaxed), 100);
        assert_eq!(volumes.local.load(Ordering::Relaxed), 0);
    }
}

#[cfg(test)]
mod overlap_tests;
