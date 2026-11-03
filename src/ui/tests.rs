use super::*;
use raspiboard::hotkeys::{Assignment, Event as HotkeyEvent};
use raspiboard::theme::ThemeChoice;

#[test]
fn hotkeys_register_only_enabled_sounds_and_confirm_the_desktop_assignment() {
    let mut board = default_board();
    assert!(board.hotkeys.config.bindings.is_empty());
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    options_event(&mut board, button_options::Action::SetHotkey);
    assert_eq!(board.hotkeys.config.bindings.len(), 1);
    assert_eq!(board.hotkeys.config.configure.as_deref(), Some(INITIAL_ID));
    assert!(board.buttons[0].hotkey.is_none());
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Assigned {
        revision,
        shortcuts: vec![Assignment {
            id: INITIAL_ID.into(),
            trigger: Some("Alt+H".into()),
        }],
    }));
    assert_eq!(board.buttons[0].hotkey.as_deref(), Some("Alt+H"));
    assert!(!board.hotkeys.pending);
    assert_eq!(board.hotkeys.config.revision, revision);
}

#[test]
fn cleared_hotkeys_ignore_late_assignment_and_activation_events() {
    let mut board = default_board();
    board.buttons[0].hotkey = Some("Alt+H".into());
    board.buttons[0].full_path = Some("/sounds/horn.wav".into());
    board.audio.connected = true;
    board.sync_hotkeys(None);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    options_event(&mut board, button_options::Action::ClearHotkey);
    assert!(board.buttons[0].hotkey.is_none());
    assert!(board.hotkeys.config.bindings.is_empty());
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Assigned {
        revision,
        shortcuts: vec![Assignment {
            id: INITIAL_ID.into(),
            trigger: Some("Alt+H".into()),
        }],
    }));
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Activated {
        revision,
        id: INITIAL_ID.into(),
    }));
    assert!(board.buttons[0].hotkey.is_none());
    assert!(board.error.is_none());
}

#[test]
fn hotkeys_play_their_sound_without_requiring_a_focused_window() {
    let mut board = default_board();
    board.buttons.push(Button {
        id: "bell".into(),
        full_path: Some("/sounds/bell.wav".into()),
        hotkey: Some("Ctrl+F2".into()),
        ..Button::default()
    });
    board.audio.connected = true;
    board.sync_hotkeys(None);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Activated {
        revision,
        id: "bell".into(),
    }));
    assert_eq!(board.active_button.as_deref(), Some("bell"));
    // Reaching the existing playback path attempts to send to the audio worker.
    assert!(board.error.is_some());
    board.error = None;
    board.active_button = Some(INITIAL_ID.into());
    board.audio.connected = false;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Activated {
        revision,
        id: "bell".into(),
    }));
    assert_eq!(board.active_button.as_deref(), Some(INITIAL_ID));
    assert!(board.error.is_none());
}

#[test]
fn cancelling_setup_restores_existing_bindings_without_erasing_their_keys() {
    let mut board = default_board();
    board.buttons[0].hotkey = Some("Ctrl+F1".into());
    board.buttons.push(Button {
        id: "bell".into(),
        ..Button::default()
    });
    let _ = board.update(Message::OpenButtonOptions("bell".into()));
    options_event(&mut board, button_options::Action::SetHotkey);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Cancelled { revision }));
    assert_eq!(board.buttons[0].hotkey.as_deref(), Some("Ctrl+F1"));
    assert!(board.buttons[1].hotkey.is_none());
    assert_eq!(board.hotkeys.config.bindings.len(), 1);
    assert_eq!(board.hotkeys.config.bindings[0].id, INITIAL_ID);
    assert!(board.hotkeys.config.configure.is_none());
}

#[test]
fn deleting_a_sound_invalidates_its_pending_shortcut_setup() {
    let mut board = default_board();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    options_event(&mut board, button_options::Action::SetHotkey);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::DeleteButton(INITIAL_ID.into()));
    assert!(board.hotkeys.config.bindings.is_empty());
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Assigned {
        revision,
        shortcuts: vec![Assignment {
            id: INITIAL_ID.into(),
            trigger: Some("Alt+H".into()),
        }],
    }));
    assert!(board.buttons.is_empty());
}

#[test]
fn portal_errors_preserve_assignments_and_stale_errors_are_ignored() {
    let mut board = default_board();
    board.buttons[0].hotkey = Some("Ctrl+F1".into());
    board.sync_hotkeys(None);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Failed {
        revision: revision - 1,
        error: "stale".into(),
    }));
    assert!(board.hotkeys.error.is_none());
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Failed {
        revision,
        error: "Portal unavailable".into(),
    }));
    assert_eq!(board.hotkeys.error.as_deref(), Some("Portal unavailable"));
    assert_eq!(board.buttons[0].hotkey.as_deref(), Some("Ctrl+F1"));
    assert!(!board.hotkeys.pending);
}

#[test]
fn failed_addition_restores_other_shortcuts_once_and_keeps_the_error_visible() {
    let mut board = default_board();
    board.buttons[0].hotkey = Some("Ctrl+F1".into());
    board.buttons.push(Button {
        id: "bell".into(),
        ..Button::default()
    });
    let _ = board.update(Message::OpenButtonOptions("bell".into()));
    options_event(&mut board, button_options::Action::SetHotkey);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Failed {
        revision,
        error: "Registration failed".into(),
    }));
    assert_eq!(board.hotkeys.config.bindings.len(), 1);
    assert_eq!(board.hotkeys.config.bindings[0].id, INITIAL_ID);
    assert!(board.hotkeys.config.configure.is_none());
    assert_eq!(board.hotkeys.error.as_deref(), Some("Registration failed"));
    let fallback_revision = board.hotkeys.config.revision;
    assert_ne!(fallback_revision, revision);
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Assigned {
        revision: fallback_revision,
        shortcuts: vec![Assignment {
            id: INITIAL_ID.into(),
            trigger: Some("Ctrl+F1".into()),
        }],
    }));
    assert_eq!(board.hotkeys.error.as_deref(), Some("Registration failed"));
    let _ = board.update(Message::Hotkeys(HotkeyEvent::Failed {
        revision: fallback_revision,
        error: "Still unavailable".into(),
    }));
    assert_eq!(board.hotkeys.config.revision, fallback_revision);
    assert_eq!(board.buttons[0].hotkey.as_deref(), Some("Ctrl+F1"));
}

#[test]
fn chooser_failure_keeps_the_successfully_registered_session() {
    let mut board = default_board();
    board.buttons[0].hotkey = Some("Ctrl+F1".into());
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    options_event(&mut board, button_options::Action::SetHotkey);
    let revision = board.hotkeys.config.revision;
    let _ = board.update(Message::Hotkeys(HotkeyEvent::ConfigureFailed {
        revision,
        error: "Use desktop settings".into(),
    }));
    assert_eq!(board.hotkeys.config.revision, revision);
    assert_eq!(board.hotkeys.config.bindings[0].id, INITIAL_ID);
    assert_eq!(board.hotkeys.error.as_deref(), Some("Use desktop settings"));
}

#[test]
fn resizing_sound_buttons_clamps_limits_and_preserves_playback() {
    let mut board = selected_board();
    board.audio.connected = true;
    board.audio.playback = PlaybackStatus::Playing;
    board.buttons[0].full_path = Some("/sounds/horn.wav".into());
    let selected = board.audio.selected.clone();
    let _ = board.update(Message::SoundButtonScaleChanged(0));
    assert_eq!(board.sound_button_scale, 50);
    assert_eq!(board.button_dimensions(), iced::Size::new(90.0, 43.0));
    let _ = board.update(Message::SoundButtonScaleChanged(999));
    assert_eq!(board.sound_button_scale, 200);
    assert_eq!(board.button_dimensions(), iced::Size::new(360.0, 172.0));
    assert_eq!(board.audio.playback, PlaybackStatus::Playing);
    assert_eq!(board.audio.selected, selected);
    assert!(board.can_play());
}

#[test]
fn application_choices_hide_descriptions_and_sources_but_keep_stream_ids() {
    for label in [
        "Recorder [20] — microphone",
        "Recorder — Capture audio [20] — source unknown",
        "Recorder",
    ] {
        let choice = TargetChoice(Some(Target {
            id: "pipewire:20:200".into(),
            display_label: "Recorder [20]".into(),
            label: label.into(),
        }));
        assert_eq!(choice.to_string(), "Recorder [20]");
    }
    assert_eq!(TargetChoice(None).to_string(), "No Output");
}

#[test]
fn repeated_settings_clicks_reuse_the_window_and_native_close_allows_reopening() {
    let mut board = default_board();
    let _ = board.update(Message::OpenSettings);
    let first = board.settings_window.unwrap();
    assert_eq!(board.title(first), "Raspiboard — Settings");
    let _ = board.update(Message::OpenSettings);
    assert_eq!(board.settings_window, Some(first));
    let _ = board.update(Message::WindowClosed(first));
    assert!(board.settings_window.is_none());
    let _ = board.update(Message::OpenSettings);
    assert_ne!(board.settings_window, Some(first));
    // A delayed close event for an older window cannot clear its replacement.
    let replacement = board.settings_window;
    let _ = board.update(Message::WindowClosed(first));
    assert_eq!(board.settings_window, replacement);
}

#[test]
fn settings_window_does_not_block_the_main_grid_and_escape_is_window_specific() {
    let mut board = default_board();
    let main = iced::window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::OpenSettings);
    let settings = board.settings_window.unwrap();
    let _ = board.update(Message::OpenGridMenu);
    assert!(board.grid_menu.is_some());
    let _ = board.update(Message::EscapePressed(main));
    assert!(board.grid_menu.is_none());
    assert_eq!(board.settings_window, Some(settings));
    let _ = board.update(Message::AddSound);
    assert!(board.choosing_file.is_some());
    let _ = board.update(Message::RemoteVolumeChanged(35));
    assert_eq!(board.remote_volume, 35);
    let _ = board.update(Message::EscapePressed(settings));
    let _ = board.update(Message::WindowClosed(settings));
    assert!(board.settings_window.is_none());
}

#[test]
fn closing_main_window_clears_window_state_and_requests_application_exit() {
    let mut board = default_board();
    let main = iced::window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::OpenSettings);
    let _ = board.update(Message::WindowClosed(main));
    assert!(board.main_window.is_none());
    assert!(board.settings_window.is_none());
}

#[test]
fn grid_menu_uses_the_click_location_and_dismisses_without_adding() {
    let mut board = default_board();
    let original = board.buttons.clone();
    let point = Point::new(240.0, 160.0);
    let _ = board.update(Message::GridPointerMoved(point));
    let _ = board.update(Message::OpenGridMenu);
    let _ = board.update(Message::GridPointerMoved(Point::ORIGIN));
    assert_eq!(board.grid_menu, Some(point));
    let _ = board.update(Message::DismissMenu);
    assert!(board.grid_menu.is_none());
    assert_eq!(board.buttons, original);
    assert!(board.choosing_file.is_none());
}

#[test]
fn add_sound_allows_only_one_picker_and_no_pad_before_confirmation() {
    let mut board = default_board();
    let original = board.buttons.clone();
    let active = board.active_button.clone();
    let _ = board.update(Message::OpenGridMenu);
    let _ = board.update(Message::AddSound);
    let pending = board.choosing_file.clone().unwrap();
    assert_eq!(
        uuid::Uuid::parse_str(&pending).unwrap().get_version_num(),
        7
    );
    assert!(board.grid_menu.is_none());
    assert_eq!(board.buttons, original);
    assert_eq!(board.active_button, active);
    let _ = board.update(Message::AddSound);
    let _ = board.update(Message::OpenGridMenu);
    let _ = board.update(Message::OpenAudio(INITIAL_ID.into()));
    assert_eq!(board.choosing_file.as_deref(), Some(pending.as_str()));
    assert!(board.grid_menu.is_none());
}

fn selected_board() -> Soundboard {
    let mut board = default_board();
    board.audio.selected = Some(Target {
        id: "pipewire:20:200".into(),
        display_label: "Recorder [20]".into(),
        label: "Recorder".into(),
    });
    board
}

#[test]
fn selecting_the_current_destination_is_a_no_op() {
    let mut board = selected_board();
    let selected = board.audio.selected.clone().unwrap();
    let _ = board.update(Message::SelectTarget(Some(selected.clone())));
    assert_eq!(board.audio.selected, Some(selected));
    assert!(board.error.is_none());
}

#[test]
fn failed_destination_commands_preserve_the_confirmed_selection() {
    let mut board = selected_board();
    let selected = board.audio.selected.clone();
    let _ = board.update(Message::SelectTarget(None));
    assert_eq!(board.audio.selected, selected);
    assert!(board.error.is_some());
}

#[test]
fn choosing_none_before_discovery_still_sends_the_explicit_choice() {
    let mut board = default_board();
    let _ = board.update(Message::SelectTarget(None));
    assert!(board.audio.selected.is_none());
    assert!(board.error.is_some());
}

#[test]
fn unlinked_volume_edits_are_independent() {
    let mut board = default_board();
    let _ = board.update(Message::RemoteVolumeChanged(25));
    let _ = board.update(Message::LocalVolumeChanged(70));
    assert_eq!((board.remote_volume, board.local_volume), (25, 70));
    assert!(!board.volumes_linked);
    assert!(board.error.is_none());
}

#[test]
fn linked_volume_edits_work_from_either_slider() {
    let mut board = default_board();
    let _ = board.update(Message::ToggleVolumeLink);
    let _ = board.update(Message::RemoteVolumeChanged(25));
    assert_eq!((board.remote_volume, board.local_volume), (25, 25));
    let _ = board.update(Message::LocalVolumeChanged(60));
    assert_eq!((board.remote_volume, board.local_volume), (60, 60));
}

#[test]
fn linking_uses_remote_level_and_unlinking_retains_both_levels() {
    let mut board = default_board();
    let _ = board.update(Message::RemoteVolumeChanged(25));
    let _ = board.update(Message::LocalVolumeChanged(80));
    let _ = board.update(Message::ToggleVolumeLink);
    assert_eq!((board.remote_volume, board.local_volume), (25, 25));
    let _ = board.update(Message::ToggleVolumeLink);
    assert_eq!((board.remote_volume, board.local_volume), (25, 25));
    let _ = board.update(Message::LocalVolumeChanged(0));
    assert_eq!((board.remote_volume, board.local_volume), (25, 0));
    let _ = board.update(Message::ToggleVolumeLink);
    assert_eq!((board.remote_volume, board.local_volume), (25, 25));
}

#[test]
fn canceling_assignment_preserves_the_sound() {
    let mut board = Soundboard {
        choosing_file: Some(INITIAL_ID.into()),
        ..default_board()
    };
    board.buttons[0].full_path = Some("trumpet.wav".into());
    let _ = board.update(Message::AudioChosen {
        id: INITIAL_ID.into(),
        path: None,
    });
    assert_eq!(board.buttons[0].full_path, Some("trumpet.wav".into()));
    assert!(board.choosing_file.is_none());
}

#[test]
fn assigning_a_sound_does_not_start_playback() {
    let mut board = selected_board();
    board.choosing_file = Some(INITIAL_ID.into());
    let _ = board.update(Message::AudioChosen {
        id: INITIAL_ID.into(),
        path: Some("new sound.wav".into()),
    });
    assert_eq!(board.buttons[0].full_path, Some("new sound.wav".into()));
    assert_eq!(board.audio.playback, PlaybackStatus::Idle);
    assert!(board.choosing_file.is_none());
    assert!(board.error.is_none());
}

#[test]
fn repeated_right_click_does_not_open_another_dialog() {
    let mut board = Soundboard {
        choosing_file: Some(INITIAL_ID.into()),
        ..default_board()
    };
    board.buttons[0].full_path = Some("trumpet.wav".into());
    let _ = board.update(Message::OpenAudio(INITIAL_ID.into()));
    assert_eq!(board.choosing_file.as_deref(), Some(INITIAL_ID));
    assert_eq!(board.buttons[0].full_path, Some("trumpet.wav".into()));
    assert!(board.error.is_none());
}

#[test]
fn play_needs_a_file_and_pipewire_connection_but_no_application_target() {
    let mut board = selected_board();
    board.buttons[0].full_path = Some("trumpet.wav".into());
    assert!(!board.can_play());
    let _ = board.update(Message::Play);
    assert!(board.error.is_none());
    board.audio.connected = true;
    assert!(board.can_play());
    board.choosing_file = Some(INITIAL_ID.into());
    assert!(!board.can_play());
    let _ = board.update(Message::Play);
    assert!(board.error.is_none());
    board.choosing_file = None;
    board.audio.selected = None;
    assert!(board.can_play());
    board.audio.selected = selected_board().audio.selected;
    board.buttons[0].full_path = None;
    assert!(!board.can_play());
}

#[test]
fn theme_selection_applies_immediately_and_survives_reopening_settings() {
    let mut board = default_board();
    assert_eq!(board.theme(), ThemeChoice::Deck.iced());

    let _ = board.update(Message::OpenSettings);
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Console));
    assert!(board.settings_window.is_some());
    assert_eq!(board.theme(), ThemeChoice::Console.iced());

    let _ = board.update(Message::CloseSettings);
    let settings = board.settings_window.unwrap();
    let _ = board.update(Message::WindowClosed(settings));
    assert!(board.settings_window.is_none());
    assert_eq!(board.theme(), ThemeChoice::Console.iced());

    let _ = board.update(Message::OpenSettings);
    assert_eq!(board.theme(), ThemeChoice::Console.iced());
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Deck));
    assert!(board.settings_window.is_some());
    assert_eq!(board.theme(), ThemeChoice::Deck.iced());
}

#[test]
fn changing_the_theme_preserves_playback_and_volume_state() {
    let mut board = selected_board();
    board.audio.connected = true;
    board.audio.playback = PlaybackStatus::Playing;
    board.buttons[0].full_path = Some("trumpet.wav".into());
    board.remote_volume = 25;
    board.local_volume = 70;
    let target = board.audio.selected.clone();

    let _ = board.update(Message::OpenSettings);
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Console));
    let _ = board.update(Message::CloseSettings);

    assert_eq!(board.theme(), ThemeChoice::Console.iced());
    assert_eq!(board.audio.playback, PlaybackStatus::Playing);
    assert_eq!(board.audio.selected, target);
    assert_eq!(board.buttons[0].full_path, Some("trumpet.wav".into()));
    assert_eq!((board.remote_volume, board.local_volume), (25, 70));
    assert!(!board.volumes_linked);
    assert!(board.can_play());
    assert!(board.error.is_none());
}

const INITIAL_ID: &str = "01a19322-a325-71a8-b5ad-76fcabdba7d8";

fn default_board() -> Soundboard {
    let mut board = Soundboard::default();
    board.buttons[0].id = INITIAL_ID.into();
    board.active_button = Some(INITIAL_ID.into());
    board
}

#[test]
fn transport_offers_pause_and_resume_without_restarting_the_clip() {
    let mut board = selected_board();
    board.audio.connected = true;
    board.buttons[0].full_path = Some("/sounds/horn.wav".into());
    assert!(matches!(board.transport_action(), Some(Message::Play)));
    board.audio.playback = PlaybackStatus::Loading;
    assert!(board.transport_action().is_none());
    board.audio.playback = PlaybackStatus::Playing;
    assert!(matches!(board.transport_action(), Some(Message::Pause)));
    board.audio.playback = PlaybackStatus::Paused;
    assert!(matches!(board.transport_action(), Some(Message::Resume)));
    board.audio.connected = false;
    assert!(board.transport_action().is_none());
}

#[test]
fn pad_menu_opens_without_stopping_audio_or_starting_a_picker() {
    let mut board = default_board();
    board.audio.playback = PlaybackStatus::Playing;
    let point = Point::new(120.0, 80.0);
    let _ = board.update(Message::GridPointerMoved(point));
    let _ = board.update(Message::OpenButtonMenu(INITIAL_ID.into()));
    assert_eq!(board.button_menu.as_deref(), Some(INITIAL_ID));
    assert_eq!(board.grid_menu, Some(point));
    assert!(board.choosing_file.is_none());
    assert_eq!(board.audio.playback, PlaybackStatus::Playing);
    let _ = board.update(Message::DismissMenu);
    assert!(board.grid_menu.is_none());
    assert!(board.button_menu.is_none());
}

#[test]
fn button_options_reuses_window_and_edits_only_its_selected_button() {
    let mut board = default_board();
    let original = board.buttons[0].clone();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    let window = board.options_window.unwrap();
    assert_eq!(board.title(window), "Raspiboard — Sound options");
    let _ = board.update(Message::ButtonNameChanged {
        id: INITIAL_ID.into(),
        name: "Door bell".into(),
    });
    assert_eq!(board.buttons[0].name.as_deref(), Some("Door bell"));
    assert_eq!(board.buttons[0].full_path, original.full_path);
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    assert_eq!(board.options_window, Some(window));
    let _ = board.update(Message::ButtonNameChanged {
        id: "missing".into(),
        name: "Wrong".into(),
    });
    assert_eq!(board.buttons[0].name.as_deref(), Some("Door bell"));
    let _ = board.update(Message::ButtonNameChanged {
        id: INITIAL_ID.into(),
        name: "".into(),
    });
    assert_eq!(board.buttons[0].name, None);
    let _ = board.update(Message::WindowClosed(window));
    assert!(board.options_window.is_none());
    assert!(board.options_button.is_none());
}

#[test]
fn deleting_a_pad_invalidates_its_pending_picker_and_options() {
    let mut board = default_board();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    board.choosing_file = Some(INITIAL_ID.into());
    let _ = board.update(Message::DeleteButton(INITIAL_ID.into()));
    assert!(board.buttons.is_empty());
    assert_eq!(board.default_button_count, 0);
    assert!(board.active_button.is_none());
    assert!(board.options_window.is_none());
    assert!(board.choosing_file.is_none());
    let _ = board.update(Message::AudioChosen {
        id: INITIAL_ID.into(),
        path: Some("/sounds/deleted.wav".into()),
    });
    let _ = board.update(Message::ButtonNameChanged {
        id: INITIAL_ID.into(),
        name: "stale".into(),
    });
    assert!(board.buttons.is_empty());
}

fn options_event(board: &mut Soundboard, action: button_options::Action) {
    let generation = board.options.generation;
    let _ = board.update(Message::OptionsChanged { generation, action });
}

#[test]
fn options_crop_drafts_and_drag_preview_only_save_valid_committed_ranges() {
    let mut board = default_board();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    board.options.waveform = Some(std::sync::Arc::new(audio::waveform::Waveform {
        peaks: vec![0.5],
        duration_ms: 3000,
    }));
    options_event(&mut board, button_options::Action::Start("1".into()));
    assert_eq!(board.buttons[0].start_ms, Some(1000));
    options_event(&mut board, button_options::Action::Start("1.".into()));
    assert_eq!(board.options.start, "1.");
    assert!(board.options.crop_error.is_some());
    assert_eq!(board.buttons[0].start_ms, Some(1000));
    options_event(
        &mut board,
        button_options::Action::Range {
            start: 500,
            end: 2000,
            commit: false,
        },
    );
    assert_eq!(board.options.start, "0.5");
    assert_eq!(board.buttons[0].start_ms, Some(1000));
    options_event(
        &mut board,
        button_options::Action::Range {
            start: 500,
            end: 2000,
            commit: true,
        },
    );
    assert_eq!(
        (board.buttons[0].start_ms, board.buttons[0].end_ms),
        (Some(500), Some(2000))
    );
    options_event(
        &mut board,
        button_options::Action::Range {
            start: 0,
            end: 3000,
            commit: true,
        },
    );
    assert_eq!(
        (board.buttons[0].start_ms, board.buttons[0].end_ms),
        (None, None)
    );
    options_event(&mut board, button_options::Action::End("4".into()));
    assert!(board.options.crop_error.is_some());
    assert_eq!(board.buttons[0].end_ms, None);
    options_event(&mut board, button_options::Action::ResetCrop);
    assert!(board.options.crop_error.is_none());
    assert!(board.options.start.is_empty());
    assert!(board.options.end.is_empty());
}

#[test]
fn options_color_and_gain_do_not_restart_audio_and_reset_to_defaults() {
    let mut board = default_board();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    board.audio.playback = PlaybackStatus::Paused;
    options_event(
        &mut board,
        button_options::Action::ColorPicked(iced::color!(0x22cc88)),
    );
    options_event(&mut board, button_options::Action::Gain(-4.5));
    assert_eq!(board.buttons[0].color.as_deref(), Some("#22CC88"));
    assert_eq!(board.buttons[0].relative_volume, Some(-4.5));
    assert_eq!(board.audio.playback, PlaybackStatus::Paused);
    options_event(&mut board, button_options::Action::Gain(f32::NAN));
    assert_eq!(board.buttons[0].relative_volume, Some(-4.5));
    options_event(&mut board, button_options::Action::ResetColor);
    options_event(&mut board, button_options::Action::Gain(0.0));
    assert_eq!(board.buttons[0].color, None);
    assert_eq!(board.buttons[0].relative_volume, None);
}

#[test]
fn options_ignore_stale_analysis_and_cancel_on_close_or_replacement() {
    let mut board = default_board();
    let old_path = PathBuf::from("/old.wav");
    board.buttons[0].full_path = Some(old_path.clone());
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    let generation = board.options.generation;
    let cancel = board.options.cancel.as_ref().unwrap().clone();
    let waveform = std::sync::Arc::new(audio::waveform::Waveform {
        peaks: vec![0.5],
        duration_ms: 1000,
    });
    board.buttons[0].full_path = Some("/new.wav".into());
    board.options_analyzed(generation, INITIAL_ID, &old_path, Ok(waveform.clone()));
    assert!(board.options.waveform.is_none());
    let _ = board.reload_options();
    assert!(cancel.load(std::sync::atomic::Ordering::Acquire));
    let new_generation = board.options.generation;
    board.options_analyzed(
        generation,
        INITIAL_ID,
        Path::new("/new.wav"),
        Ok(waveform.clone()),
    );
    assert!(board.options.waveform.is_none());
    board.options_analyzed(
        new_generation,
        INITIAL_ID,
        Path::new("/new.wav"),
        Ok(waveform.clone()),
    );
    assert_eq!(board.options.duration(), Some(1000));
    let cancel = board.options.cancel.as_ref().unwrap().clone();
    let _ = board.update(Message::CloseButtonOptions);
    assert!(cancel.load(std::sync::atomic::Ordering::Acquire));
    let _ = board.update(Message::OptionsChanged {
        generation: new_generation,
        action: button_options::Action::Gain(10.0),
    });
    assert_eq!(board.buttons[0].relative_volume, None);
}

#[test]
fn options_preview_uses_current_crop_and_gain_and_requires_valid_input() {
    let mut board = selected_board();
    board.audio.connected = true;
    board.buttons[0].full_path = Some("/clip.wav".into());
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    board.options.waveform = Some(std::sync::Arc::new(audio::waveform::Waveform {
        peaks: vec![0.5],
        duration_ms: 3000,
    }));
    options_event(&mut board, button_options::Action::Start("0.5".into()));
    options_event(&mut board, button_options::Action::End("2".into()));
    options_event(&mut board, button_options::Action::Gain(-3.0));
    let command = board.preview_command().unwrap();
    match command {
        Command::Preview {
            button_id,
            path,
            options,
        } => {
            assert_eq!(button_id, INITIAL_ID);
            assert_eq!(path, PathBuf::from("/clip.wav"));
            assert_eq!(options.start_ms, Some(500));
            assert_eq!(options.end_ms, Some(2000));
            assert_eq!(options.relative_volume, Some(-3.0));
        }
        _ => panic!("preview must use the local-only command"),
    }
    options_event(&mut board, button_options::Action::End("0.2".into()));
    assert!(board.preview_command().is_none());
    board.audio.playback = PlaybackStatus::Playing;
    board.audio.playbacks = vec![audio::PlaybackInstance {
        button_id: INITIAL_ID.into(),
        status: PlaybackStatus::Playing,
        preview: true,
    }];
    assert!(board.preview_playing());
    board.audio.playback = PlaybackStatus::Idle;
    board.audio.playbacks.clear();
    assert!(!board.preview_playing());
}

#[test]
fn custom_theme_changes_keep_options_drafts_picker_and_audio_state() {
    let mut board = default_board();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    board.options.start = "1.".into();
    board.options.end = "2.5".into();
    board.options.color_picker = true;
    board.audio.playback = PlaybackStatus::Paused;
    let generation = board.options.generation;
    let window = board.options_window;
    let buttons = board.buttons.clone();
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Console));
    assert_eq!(board.theme().to_string(), "Console");
    assert_eq!(board.options.start, "1.");
    assert_eq!(board.options.end, "2.5");
    assert!(board.options.color_picker);
    assert_eq!(board.options.generation, generation);
    assert_eq!(board.options_window, window);
    assert_eq!(board.audio.playback, PlaybackStatus::Paused);
    assert_eq!(board.buttons, buttons);
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Deck));
    assert_eq!(board.theme().to_string(), "Deck");
    assert_eq!(ThemeChoice::ALL, &[ThemeChoice::Deck, ThemeChoice::Console]);
}

#[test]
fn playback_commands_include_button_identity_overlap_crop_and_gain() {
    let mut board = selected_board();
    board.audio.connected = true;
    board.buttons[0].full_path = Some("/clip.wav".into());
    board.buttons[0].overlap = true;
    board.buttons[0].start_ms = Some(100);
    board.buttons[0].end_ms = Some(900);
    board.buttons[0].relative_volume = Some(-3.0);
    match board.play_command(INITIAL_ID).unwrap() {
        Command::Play {
            button_id,
            overlap,
            path,
            options,
            target,
        } => {
            assert_eq!(button_id, INITIAL_ID);
            assert!(overlap);
            assert_eq!(path, PathBuf::from("/clip.wav"));
            assert_eq!(options.start_ms, Some(100));
            assert_eq!(options.end_ms, Some(900));
            assert_eq!(options.relative_volume, Some(-3.0));
            assert_eq!(target, board.audio.selected);
        }
        _ => panic!("expected pad playback"),
    }
    board.buttons[0].overlap = false;
    assert!(matches!(
        board.play_command(INITIAL_ID),
        Some(Command::Play { overlap: false, .. })
    ));
}

#[test]
fn every_playing_pad_is_highlighted_independently_of_the_selected_pad() {
    let mut board = default_board();
    board.audio.playback = PlaybackStatus::Playing;
    board.audio.playbacks = vec![
        audio::PlaybackInstance {
            button_id: "horn".into(),
            status: PlaybackStatus::Playing,
            preview: false,
        },
        audio::PlaybackInstance {
            button_id: "bell".into(),
            status: PlaybackStatus::Playing,
            preview: false,
        },
        audio::PlaybackInstance {
            button_id: "loading".into(),
            status: PlaybackStatus::Loading,
            preview: false,
        },
        audio::PlaybackInstance {
            button_id: "paused".into(),
            status: PlaybackStatus::Paused,
            preview: false,
        },
    ];
    assert!(board.button_playing("horn"));
    assert!(board.button_playing("bell"));
    assert!(!board.button_playing("loading"));
    assert!(!board.button_playing("paused"));
    assert!(!board.button_playing(INITIAL_ID));
    board.audio.playbacks.remove(0);
    assert!(!board.button_playing("horn"));
    assert!(board.button_playing("bell"));
}

#[test]
fn preview_controls_identify_preview_instead_of_last_selected_sound() {
    let mut board = default_board();
    let _ = board.update(Message::OpenButtonOptions(INITIAL_ID.into()));
    board.audio.playback = PlaybackStatus::Playing;
    board.audio.playbacks = vec![audio::PlaybackInstance {
        button_id: INITIAL_ID.into(),
        status: PlaybackStatus::Playing,
        preview: false,
    }];
    assert!(!board.preview_playing());
    board.audio.playbacks[0].preview = true;
    board.active_button = Some("other".into());
    assert!(board.preview_playing());
    board.audio.playbacks.clear();
    assert!(!board.preview_playing());
}

#[test]
fn deleting_an_older_overlapping_pad_sends_stop_for_the_group() {
    let mut board = default_board();
    board.active_button = Some("other".into());
    board.audio.playback = PlaybackStatus::Playing;
    board.audio.playbacks = vec![audio::PlaybackInstance {
        button_id: INITIAL_ID.into(),
        status: PlaybackStatus::Playing,
        preview: false,
    }];
    let _ = board.update(Message::DeleteButton(INITIAL_ID.into()));
    assert!(
        board.error.is_some(),
        "deletion must attempt to stop the audio worker"
    );
}

#[test]
fn target_choice_displays_backend_label_without_interpreting_identity() {
    let target = Target {
        id: "default-microphone".into(),
        display_label: "Default microphone".into(),
        label: "Microphone details".into(),
    };
    assert_eq!(TargetChoice(Some(target)).to_string(), "Default microphone");
}

#[test]
fn close_to_tray_keeps_playback_and_restores_the_board() {
    let mut board = default_board();
    let main = window::Id::unique();
    board.main_window = Some(main);
    let buttons = board.buttons.clone();
    board.audio.playback = PlaybackStatus::Playing;
    let _ = board.update(Message::MinimizeToTrayChanged(true));
    let _ = board.update(Message::Tray(tray::Event::Available(true)));
    let _ = board.update(Message::WindowCloseRequested(main));
    assert!(board.main_hidden);
    assert!(board.main_window.is_none());
    assert_eq!(board.buttons, buttons);
    assert_eq!(board.audio.playback, PlaybackStatus::Playing);
    let _ = board.update(Message::WindowClosed(main));
    assert!(board.main_hidden);
    let _ = board.update(Message::Tray(tray::Event::Show));
    let restored = board.main_window;
    let _ = board.update(Message::WindowClosed(main));
    let _ = board.update(Message::Tray(tray::Event::Show));
    assert_eq!(board.main_window, restored);
    assert!(!board.main_hidden);
    assert!(board.main_window.is_some());
}

#[test]
fn close_requests_exit_without_a_tray_or_when_the_option_is_off() {
    for (enabled, available) in [(true, false), (false, true), (false, false)] {
        let mut board = default_board();
        let main = window::Id::unique();
        board.main_window = Some(main);
        let _ = board.update(Message::MinimizeToTrayChanged(enabled));
        let _ = board.update(Message::Tray(tray::Event::Available(available)));
        let _ = board.update(Message::WindowCloseRequested(main));
        assert!(board.main_window.is_none());
        assert!(!board.main_hidden);
    }
}

#[test]
fn tray_loss_restores_a_hidden_window_and_quit_exits_even_when_enabled() {
    let mut board = default_board();
    let main = window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::MinimizeToTrayChanged(true));
    let _ = board.update(Message::Tray(tray::Event::Available(true)));
    let _ = board.update(Message::WindowCloseRequested(main));
    let _ = board.update(Message::Tray(tray::Event::Available(false)));
    assert!(!board.main_hidden);
    assert!(board.main_window.is_some());
    let _ = board.update(Message::Tray(tray::Event::Quit));
    assert!(board.main_window.is_none());
}
