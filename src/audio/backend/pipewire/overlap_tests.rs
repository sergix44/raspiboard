use super::*;
use crate::audio::PlaybackOptions;

fn long_tone() -> std::path::PathBuf {
    static PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-results/overlap/long-tone.wav");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let data_size = 32768_u32 * 2;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_size).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(48000_u32.to_le_bytes());
        bytes.extend(96000_u32.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(data_size.to_le_bytes());
        for _ in 0..32768 {
            bytes.extend(16384_i16.to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        path
    })
    .clone()
}

fn play(id: &str, overlap: bool) -> Command {
    Command::Play {
        button_id: id.into(),
        overlap,
        path: long_tone(),
        target: None,
        options: PlaybackOptions::default(),
    }
}

fn ready(engine: &Engine) -> Vec<crate::audio::job::DecodedOutputs> {
    engine
        .sessions
        .iter()
        .map(|session| {
            let output = session
                .job
                .ready
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while output.local.samples.slots() < 16384 {
                assert!(
                    Instant::now() < deadline,
                    "decoder did not fill its output queue"
                );
                thread::yield_now();
            }
            assert!(matches!(
                output.local.result.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
            output
        })
        .collect()
}

fn cancelled(outputs: Vec<crate::audio::job::DecodedOutputs>) {
    for output in outputs {
        output
            .local
            .result
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        if let Some(remote) = output.remote {
            remote
                .result
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap();
        }
    }
}

#[test]
fn five_rapid_overlap_clicks_keep_five_independent_decoders() {
    let mut engine = Engine::default();
    for _ in 0..5 {
        engine.command(play("horn", true), &OutputVolumes::default());
    }
    assert_eq!(engine.snapshot.playbacks.len(), 5);
    assert!(
        engine
            .snapshot
            .playbacks
            .iter()
            .all(|instance| instance.button_id == "horn"
                && instance.status == PlaybackStatus::Loading)
    );
    let outputs = ready(&engine);
    assert_eq!(outputs.len(), 5);
    engine.command(Command::Stop, &OutputVolumes::default());
    cancelled(outputs);
}

#[test]
fn ordinary_play_cancels_every_previous_decoder() {
    let mut engine = Engine::default();
    engine.command(play("horn", false), &OutputVolumes::default());
    engine.command(play("bell", true), &OutputVolumes::default());
    let outputs = ready(&engine);
    assert_eq!(outputs.len(), 2);
    engine.command(play("door", false), &OutputVolumes::default());
    cancelled(outputs);
    assert_eq!(engine.snapshot.playbacks.len(), 1);
    assert_eq!(engine.snapshot.playbacks[0].button_id, "door");
}

#[test]
fn stop_cancels_all_pending_copies_and_clears_the_transport() {
    let mut engine = Engine::default();
    for _ in 0..5 {
        engine.command(play("horn", true), &OutputVolumes::default());
    }
    let outputs = ready(&engine);
    engine.command(Command::Stop, &OutputVolumes::default());
    cancelled(outputs);
    assert!(engine.sessions.is_empty());
    assert!(engine.snapshot.playbacks.is_empty());
    assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
}

#[test]
fn preview_replaces_the_group_and_overlap_play_replaces_preview() {
    let mut engine = Engine::default();
    engine.command(play("horn", true), &OutputVolumes::default());
    let outputs = ready(&engine);
    engine.command(
        Command::Preview {
            button_id: "preview".into(),
            path: long_tone(),
            options: PlaybackOptions::default(),
        },
        &OutputVolumes::default(),
    );
    cancelled(outputs);
    assert_eq!(engine.snapshot.playbacks.len(), 1);
    assert!(engine.snapshot.playbacks[0].preview);
    let outputs = ready(&engine);
    assert!(outputs[0].remote.is_none());
    engine.command(play("bell", true), &OutputVolumes::default());
    cancelled(outputs);
    assert_eq!(engine.snapshot.playbacks.len(), 1);
    assert!(!engine.snapshot.playbacks[0].preview);
    assert_eq!(engine.snapshot.playbacks[0].button_id, "bell");
}

#[test]
fn selecting_a_destination_cancels_the_whole_group() {
    let mut engine = Engine::default();
    for _ in 0..3 {
        engine.command(play("horn", true), &OutputVolumes::default());
    }
    let outputs = ready(&engine);
    engine.command(Command::Select(None), &OutputVolumes::default());
    cancelled(outputs);
    assert!(engine.snapshot.playbacks.is_empty());
    assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
}

#[test]
fn pause_and_resume_reach_every_pending_instance_in_a_playing_group() {
    let mut engine = Engine::default();
    for _ in 0..3 {
        engine.command(play("horn", true), &OutputVolumes::default());
    }
    // Simulate the transport notification while keeping this test headless.
    engine.snapshot.playback = PlaybackStatus::Playing;
    engine.command(Command::Pause, &OutputVolumes::default());
    assert_eq!(engine.snapshot.playback, PlaybackStatus::Paused);
    assert!(
        engine
            .sessions
            .iter()
            .all(|session| session.paused.load(Ordering::Acquire))
    );
    engine.command(Command::Resume, &OutputVolumes::default());
    assert!(
        engine
            .sessions
            .iter()
            .all(|session| !session.paused.load(Ordering::Acquire))
    );
    assert_eq!(engine.snapshot.playback, PlaybackStatus::Loading);
}
#[test]
fn completing_or_failing_one_instance_keeps_its_paused_neighbors() {
    for error in [None, Some("broken clip".to_string())] {
        let mut engine = Engine::default();
        engine.command(play("horn", true), &OutputVolumes::default());
        engine.command(play("bell", true), &OutputVolumes::default());
        engine.command(play("horn", true), &OutputVolumes::default());
        let mut outputs = ready(&engine);
        engine.snapshot.playback = PlaybackStatus::Playing;
        engine.command(Command::Pause, &OutputVolumes::default());
        engine.finish_session(1, error.clone());
        cancelled(vec![outputs.remove(1)]);
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Paused);
        assert_eq!(engine.snapshot.playbacks.len(), 2);
        assert!(
            engine
                .snapshot
                .playbacks
                .iter()
                .all(|instance| instance.button_id == "horn")
        );
        assert_eq!(engine.snapshot.error, error);
        for output in &outputs {
            assert!(matches!(
                output.local.result.try_recv(),
                Err(mpsc::TryRecvError::Empty)
            ));
        }
        engine.finish_session(0, None);
        engine.finish_session(0, None);
        cancelled(outputs);
        assert!(engine.snapshot.playbacks.is_empty());
        assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
    }
}

#[test]
fn overlapping_start_resumes_the_previously_paused_group() {
    let mut engine = Engine::default();
    engine.command(play("horn", true), &OutputVolumes::default());
    engine.snapshot.playback = PlaybackStatus::Playing;
    engine.command(Command::Pause, &OutputVolumes::default());
    engine.command(play("bell", true), &OutputVolumes::default());
    assert_eq!(engine.snapshot.playbacks.len(), 2);
    assert!(
        engine
            .sessions
            .iter()
            .all(|session| !session.paused.load(Ordering::Acquire))
    );
}

#[test]
fn overlap_with_an_invalid_crop_reports_error_without_stopping_other_instances() {
    let mut engine = Engine::default();
    engine.command(play("horn", true), &OutputVolumes::default());
    let outputs = ready(&engine);
    let mut command = play("invalid", true);
    if let Command::Play { options, .. } = &mut command {
        options.start_ms = Some(1000);
        options.end_ms = Some(500);
    }
    engine.command(command, &OutputVolumes::default());
    assert_eq!(engine.snapshot.playbacks.len(), 1);
    assert_eq!(engine.snapshot.playbacks[0].button_id, "horn");
    assert!(engine.snapshot.error.is_some());
    assert!(matches!(
        outputs[0].local.result.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    engine.command(Command::Stop, &OutputVolumes::default());
    cancelled(outputs);
}

#[test]
fn group_transport_stays_playing_until_all_playing_instances_finish() {
    use crate::audio::PlaybackInstance;
    for (statuses, expected) in [
        (vec![], PlaybackStatus::Idle),
        (vec![PlaybackStatus::Loading], PlaybackStatus::Loading),
        (
            vec![PlaybackStatus::Paused, PlaybackStatus::Loading],
            PlaybackStatus::Paused,
        ),
        (
            vec![PlaybackStatus::Loading, PlaybackStatus::Playing],
            PlaybackStatus::Playing,
        ),
        (
            vec![PlaybackStatus::Paused, PlaybackStatus::Playing],
            PlaybackStatus::Playing,
        ),
        (
            vec![PlaybackStatus::Paused, PlaybackStatus::Paused],
            PlaybackStatus::Paused,
        ),
    ] {
        let instances: Vec<_> = statuses
            .into_iter()
            .map(|status| PlaybackInstance {
                button_id: "horn".into(),
                status,
                preview: false,
            })
            .collect();
        assert_eq!(playback_status(&instances), expected);
    }
}

#[test]
fn stop_cancels_every_decoder_and_both_outputs_when_an_application_is_selected() {
    let mut engine = Engine::default();
    let target = crate::audio::Target {
        id: "pipewire:20:200".into(),
        display_label: "Recorder [20]".into(),
        label: "Recorder".into(),
    };
    engine.snapshot.selected = Some(target.clone());
    for _ in 0..5 {
        let mut command = play("horn", true);
        if let Command::Play {
            target: requested, ..
        } = &mut command
        {
            *requested = Some(target.clone());
        }
        engine.command(command, &OutputVolumes::default());
    }
    let outputs = ready(&engine);
    assert_eq!(outputs.len(), 5);
    assert!(outputs.iter().all(|output| output.remote.is_some()));
    engine.command(Command::Stop, &OutputVolumes::default());
    cancelled(outputs);
    assert_eq!(engine.snapshot.playback, PlaybackStatus::Idle);
}

#[test]
fn resume_and_new_overlap_clear_paused_instances_even_while_another_is_finishing() {
    for new_overlap in [false, true] {
        let mut engine = Engine::default();
        engine.command(play("horn", true), &OutputVolumes::default());
        engine.command(play("tail", true), &OutputVolumes::default());
        engine.sessions[0].set_paused(true);
        // A draining tail keeps the aggregate Playing while its neighbor is paused.
        engine.snapshot.playback = PlaybackStatus::Playing;
        engine.command(
            if new_overlap {
                play("bell", true)
            } else {
                Command::Resume
            },
            &OutputVolumes::default(),
        );
        assert!(
            engine
                .sessions
                .iter()
                .all(|session| !session.paused.load(Ordering::Acquire))
        );
    }
}

#[test]
fn pipewire_object_errors_do_not_become_connection_failures() {
    assert_eq!(connection_error(42, "link failed"), None);
    assert_eq!(connection_error(17, "sink failed"), None);
    assert_eq!(
        connection_error(pipewire::core::PW_ID_CORE, "connection lost"),
        Some("connection lost".into())
    );
}
