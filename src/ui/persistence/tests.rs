use super::*;
use crate::ui::Message;
use raspiboard::audio::{PlaybackStatus, Target};
use raspiboard::storage::Category;
use raspiboard::theme::ThemeChoice;
use std::fs;

fn directory() -> tempfile::TempDir {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-results/persistence-ui");
    fs::create_dir_all(&base).unwrap();
    tempfile::tempdir_in(base).unwrap()
}

#[test]
fn desktop_hotkeys_restore_on_launch_and_clear_survives_restart() {
    use raspiboard::hotkeys::{Assignment, Event};
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let id = board.buttons[0].id.clone();
    let _ = board.update(Message::OpenButtonOptions(id.clone()));
    let generation = board.options.generation;
    let _ = board.update(Message::OptionsChanged {
        generation,
        action: button_options::Action::SetHotkey,
    });
    let _ = board.update(Message::Hotkeys(Event::Assigned {
        revision: board.hotkeys.config.revision,
        shortcuts: vec![Assignment {
            id: id.clone(),
            trigger: Some("Alt+H".into()),
        }],
    }));
    drop(board);
    let mut board = Soundboard::from_store(store.clone());
    assert_eq!(board.buttons[0].hotkey.as_deref(), Some("Alt+H"));
    assert_eq!(board.hotkeys.config.bindings[0].id, id);
    assert!(board.hotkeys.config.configure.is_none());
    let _ = board.update(Message::OpenButtonOptions(id));
    let generation = board.options.generation;
    let _ = board.update(Message::OptionsChanged {
        generation,
        action: button_options::Action::ClearHotkey,
    });
    drop(board);
    let board = Soundboard::from_store(store);
    assert!(board.buttons[0].hotkey.is_none());
    assert!(board.hotkeys.config.bindings.is_empty());
}

#[test]
fn sound_button_size_is_applied_live_and_restored_without_changing_assignments() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let buttons = board.buttons.clone();
    assert_eq!(board.sound_button_scale, 100);
    assert_eq!(board.button_dimensions(), iced::Size::new(180.0, 86.0));
    let _ = board.update(Message::SoundButtonScaleChanged(150));
    assert_eq!(board.button_dimensions(), iced::Size::new(270.0, 129.0));
    let _ = board.update(Message::RemoteVolumeChanged(35));
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Console));
    assert_eq!(board.buttons, buttons);
    assert_eq!(board.audio.playback, PlaybackStatus::Idle);
    drop(board);
    let board = Soundboard::from_store(store);
    assert_eq!(board.sound_button_scale, 150);
    assert_eq!(board.buttons, buttons);
    assert_eq!(board.theme(), ThemeChoice::Console.iced());
    assert_eq!(board.remote_volume, 35);
}

#[test]
fn adding_sounds_appends_pads_and_restores_their_order() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let original = board.buttons[0].clone();
    for (id, filename) in [(HORN, "new-horn"), (BELL, "new-bell")] {
        board.choosing_file = Some(id.into());
        let _ = board.update(Message::AudioChosen {
            id: id.into(),
            path: Some(dir.path().join(format!("{filename}.wav"))),
        });
        assert_eq!(board.active_button.as_deref(), Some(id));
        assert_eq!(board.audio.playback, PlaybackStatus::Idle);
        assert!(board.choosing_file.is_none());
    }
    let expected = board.buttons.clone();
    assert_eq!(expected.len(), 3);
    assert_eq!(expected[0], original);
    assert_eq!(expected[1].full_path, Some(dir.path().join("new-horn.wav")));
    assert_eq!(expected[2].full_path, Some(dir.path().join("new-bell.wav")));
    drop(board);
    assert_eq!(store.load_or_create().unwrap().ordered_buttons(), expected);
}

#[test]
fn canceling_add_sound_keeps_grid_and_active_button_unchanged() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let original = board.buttons.clone();
    let active = board.active_button.clone();
    board.choosing_file = Some("new-sound".into());
    let _ = board.update(Message::AudioChosen {
        id: "new-sound".into(),
        path: None,
    });
    assert_eq!(board.buttons, original);
    assert_eq!(board.active_button, active);
    assert!(board.choosing_file.is_none());
    drop(board);
    assert_eq!(store.load_or_create().unwrap().ordered_buttons(), original);
    assert_eq!(
        fs::read_dir(dir.path().join("categories")).unwrap().count(),
        1
    );
}

#[test]
fn reopening_restores_settings_and_grid_order_without_starting_audio() {
    let dir = directory();
    let store = Store::new(dir.path().to_path_buf());
    let initial = store.load_or_create().unwrap();
    let original_id = initial.ordered_buttons()[0].id.clone();
    store
        .save_settings(&Settings {
            theme: ThemeChoice::Console,
            remote_volume: 28,
            local_volume: 65,
            volumes_linked: false,
            sound_button_scale: 100,
        })
        .unwrap();
    store
        .save_button(&Button {
            id: HORN.into(),
            name: Some("Air horn".into()),
            full_path: Some(dir.path().join("original horn.wav")),
            sorting: 0,
            ..Button::default()
        })
        .unwrap();

    let board = Soundboard::from_store(store);
    assert_eq!(board.theme(), ThemeChoice::Console.iced());
    assert_eq!((board.remote_volume, board.local_volume), (28, 65));
    assert!(!board.volumes_linked);
    assert_eq!(
        board
            .buttons
            .iter()
            .map(|button| button.id.as_str())
            .collect::<Vec<_>>(),
        [HORN, original_id.as_str()]
    );
    assert_eq!(board.buttons[0].name.as_deref(), Some("Air horn"));
    assert_eq!(
        board.buttons[0].full_path,
        Some(dir.path().join("original horn.wav"))
    );
    assert_eq!(board.active_button.as_deref(), Some(HORN));
    assert_eq!(board.audio.playback, PlaybackStatus::Idle);
    assert!(board.audio.selected.is_none());
    assert!(board.settings_window.is_none());
    assert!(board.choosing_file.is_none());
    assert!(board.storage_error.is_none());
}

#[test]
fn durable_edits_are_saved_in_order_and_assignment_keeps_the_original_wav() {
    let dir = directory();
    let root = dir.path().join("config");
    let wav = dir.path().join("original sound.wav");
    fs::write(&wav, b"original WAV bytes").unwrap();
    let store = Store::new(root.clone());
    let mut board = Soundboard::from_store(store.clone());
    let initial_id = board.buttons[0].id.clone();
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Console));
    let _ = board.update(Message::RemoteVolumeChanged(28));
    let _ = board.update(Message::LocalVolumeChanged(65));
    let _ = board.update(Message::ToggleVolumeLink);
    board.choosing_file = Some(initial_id.clone());
    let _ = board.update(Message::AudioChosen {
        id: initial_id,
        path: Some(wav.clone()),
    });
    drop(board);

    let saved = store.load_or_create().unwrap();
    assert_eq!(saved.settings.theme, ThemeChoice::Console);
    assert_eq!(
        (saved.settings.remote_volume, saved.settings.local_volume),
        (28, 28)
    );
    assert!(saved.settings.volumes_linked);
    assert_eq!(saved.ordered_buttons()[0].full_path, Some(wav.clone()));
    assert_eq!(fs::read(wav).unwrap(), b"original WAV bytes");
    let mut entries = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(entries, ["categories", "settings.toml"]);
}

#[test]
fn assigning_one_button_does_not_change_its_neighbor_and_cancel_does_not_save() {
    let dir = directory();
    let store = Store::new(dir.path().to_path_buf());
    let initial = store.load_or_create().unwrap();
    let initial_id = initial.ordered_buttons()[0].id.clone();
    let neighbor = Button {
        id: BELL.into(),
        name: None,
        full_path: Some(dir.path().join("neighbor.wav")),
        sorting: 2,
        start_ms: Some(150),
        end_ms: Some(450),
        color: Some("#ccffee".into()),
        relative_volume: Some(0.4),
        overlap: true,
        hotkey: Some("Ctrl+B".into()),
    };
    store.save_button(&neighbor).unwrap();
    let mut board = Soundboard::from_store(store.clone());
    board.choosing_file = Some(initial_id.clone());
    let _ = board.update(Message::AudioChosen {
        id: initial_id,
        path: Some(dir.path().join("horn.wav")),
    });
    board.choosing_file = Some(BELL.into());
    let _ = board.update(Message::AudioChosen {
        id: BELL.into(),
        path: None,
    });
    drop(board);
    let saved = store.load_or_create().unwrap();
    assert_eq!(
        saved.ordered_buttons()[0].full_path,
        Some(dir.path().join("horn.wav"))
    );
    assert_eq!(saved.ordered_buttons()[1], neighbor);
    let category: Category =
        serde_json::from_slice(&fs::read(dir.path().join("categories/default.json")).unwrap())
            .unwrap();
    assert_eq!(category.entries[1], neighbor);
}

#[test]
fn playback_eligibility_uses_the_requested_button_sound() {
    let mut board = Soundboard::default();
    board.buttons.push(Button {
        id: HORN.into(),
        name: None,
        full_path: Some("/sounds/horn.wav".into()),
        ..Button::default()
    });
    board.audio.connected = true;
    board.audio.selected = Some(Target {
        id: "pipewire:20:200".into(),
        display_label: "Recorder [20]".into(),
        label: "Recorder".into(),
    });
    assert!(!board.can_play());
    let (path, target) = board.playback_for(&board.buttons[1]).unwrap();
    assert_eq!(path, std::path::Path::new("/sounds/horn.wav"));
    assert_eq!(target.unwrap().id, "pipewire:20:200".into());
    let _ = board.update(Message::PlayButton(HORN.into()));
    assert_eq!(board.active_button.as_deref(), Some(HORN));
    assert!(board.can_play());
}

#[test]
fn unreadable_settings_stay_intact_after_ui_edits() {
    let dir = directory();
    let store = Store::new(dir.path().to_path_buf());
    store.load_or_create().unwrap();
    let path = dir.path().join("settings.toml");
    let invalid = b"this is not valid TOML [";
    fs::write(&path, invalid).unwrap();
    let mut board = Soundboard::from_store(store);
    assert!(board.storage_error.is_some());
    assert!(board.persistence.is_none());
    let _ = board.update(Message::ThemeChanged(ThemeChoice::Console));
    let _ = board.update(Message::RemoteVolumeChanged(28));
    assert!(board.storage_error.is_some());
    drop(board);
    assert_eq!(fs::read(path).unwrap(), invalid);
}

#[test]
fn stale_save_reports_cannot_hide_a_newer_failure() {
    let mut board = Soundboard::default();
    let _ = board.update(Message::Saved(Report {
        revision: 2,
        errors: vec!["Cannot save button".into()],
    }));
    let _ = board.update(Message::Saved(Report {
        revision: 1,
        errors: vec![],
    }));
    assert_eq!(board.storage_error.as_deref(), Some("Cannot save button"));
    let _ = board.update(Message::Saved(Report {
        revision: 3,
        errors: vec![],
    }));
    assert!(board.storage_error.is_none());
}

#[test]
fn stale_picker_results_cannot_assign_a_different_button() {
    let mut board = Soundboard::default();
    board.buttons.push(Button {
        id: BELL.into(),
        name: None,
        full_path: None,
        ..Button::default()
    });
    board.choosing_file = Some(BELL.into());
    let _ = board.update(Message::AudioChosen {
        id: HORN.into(),
        path: Some("/sounds/wrong.wav".into()),
    });
    assert!(board.buttons[0].full_path.is_none());
    assert!(board.buttons[1].full_path.is_none());
    assert_eq!(board.choosing_file.as_deref(), Some(BELL));

    let _ = board.update(Message::AudioChosen {
        id: BELL.into(),
        path: Some("/sounds/right.wav".into()),
    });
    assert_eq!(board.buttons[1].full_path, Some("/sounds/right.wav".into()));
    assert!(board.choosing_file.is_none());
}

const HORN: &str = "01a19322-a325-71a8-b5ad-76fcabdba7d8";
const BELL: &str = "01a19322-a325-71a8-b5ad-7f0fbc6dd140";

#[test]
fn adding_to_default_with_named_categories_keeps_live_and_restored_order_equal() {
    let dir = directory();
    let store = Store::new(dir.path());
    let initial = store.load_or_create().unwrap();
    let category = Category {
        id: "01a19322-a325-71a8-b5ad-6d57adb395e2".into(),
        category_name: Some("Effects".into()),
        entries: vec![Button {
            id: BELL.into(),
            sorting: 100,
            ..Button::default()
        }],
    };
    let path = dir.path().join(format!("categories/{}.json", category.id));
    fs::write(&path, serde_json::to_vec_pretty(&category).unwrap()).unwrap();
    let category_bytes = fs::read(&path).unwrap();
    let mut board = Soundboard::from_store(store.clone());
    board.choosing_file = Some(HORN.into());
    let _ = board.update(Message::AudioChosen {
        id: HORN.into(),
        path: Some(dir.path().join("horn.wav")),
    });
    assert_eq!(
        board
            .buttons
            .iter()
            .map(|button| button.id.as_str())
            .collect::<Vec<_>>(),
        [initial.ordered_buttons()[0].id.as_str(), HORN, BELL]
    );
    assert_eq!(board.buttons[1].sorting, 2);
    let live = board.buttons.clone();
    drop(board);
    assert_eq!(store.load_or_create().unwrap().ordered_buttons(), live);
    assert_eq!(fs::read(&path).unwrap(), category_bytes);
}

#[test]
fn replacing_a_wav_preserves_its_own_metadata_and_sorting() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut button = store.load_or_create().unwrap().ordered_buttons().remove(0);
    button.name = Some("Door".into());
    button.start_ms = Some(50);
    button.end_ms = Some(300);
    button.color = Some("#aa33cc".into());
    button.relative_volume = Some(0.6);
    button.overlap = true;
    button.hotkey = Some("Alt+D".into());
    button.sorting = 7;
    store.save_button(&button).unwrap();
    let mut board = Soundboard::from_store(store.clone());
    board.choosing_file = Some(button.id.clone());
    let path = dir.path().join("replacement.wav");
    let _ = board.update(Message::AudioChosen {
        id: button.id.clone(),
        path: Some(path.clone()),
    });
    button.full_path = Some(path);
    drop(board);
    assert_eq!(store.load_or_create().unwrap().ordered_buttons(), [button]);
}

#[test]
fn options_rename_and_delete_survive_reopening_without_recreating_the_last_pad() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let id = board.buttons[0].id.clone();
    let original = board.buttons[0].clone();
    let _ = board.update(Message::OpenButtonOptions(id.clone()));
    let _ = board.update(Message::ButtonNameChanged {
        id: id.clone(),
        name: "Door bell".into(),
    });
    drop(board);
    let mut board = Soundboard::from_store(store.clone());
    assert_eq!(board.buttons[0].name.as_deref(), Some("Door bell"));
    let mut restored = board.buttons[0].clone();
    restored.name = original.name.clone();
    assert_eq!(restored, original);
    let _ = board.update(Message::DeleteButton(id));
    drop(board);
    let board = Soundboard::from_store(store);
    assert!(board.buttons.is_empty());
    assert_eq!(board.default_button_count, 0);
    assert!(board.active_button.is_none());
}

#[test]
fn sound_color_gain_and_crop_edits_roundtrip_without_touching_source_audio() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let id = board.buttons[0].id.clone();
    let source = dir.path().join("original.wav");
    fs::write(&source, b"original bytes").unwrap();
    board.choosing_file = Some(id.clone());
    let _ = board.update(Message::AudioChosen {
        id: id.clone(),
        path: Some(source.clone()),
    });
    let _ = board.update(Message::OpenButtonOptions(id));
    let generation = board.options.generation;
    for action in [
        button_options::Action::ColorPicked(iced::color!(0x22cc88)),
        button_options::Action::Gain(2.5),
        button_options::Action::Start("0.1".into()),
        button_options::Action::End("1.25".into()),
    ] {
        let _ = board.update(Message::OptionsChanged { generation, action });
    }
    drop(board);
    let board = Soundboard::from_store(store);
    assert_eq!(board.buttons[0].color.as_deref(), Some("#22CC88"));
    assert_eq!(board.buttons[0].relative_volume, Some(2.5));
    assert_eq!(board.buttons[0].start_ms, Some(100));
    assert_eq!(board.buttons[0].end_ms, Some(1250));
    assert_eq!(fs::read(source).unwrap(), b"original bytes");
}

#[test]
fn mouse3_reorder_survives_restart_and_new_sounds_append_after_it() {
    let dir = directory();
    let store = Store::new(dir.path());
    let initial = store.load_or_create().unwrap().ordered_buttons()[0].clone();
    store
        .save_button(&Button {
            id: HORN.into(),
            name: Some("Horn".into()),
            sorting: 2,
            ..Button::default()
        })
        .unwrap();
    store
        .save_button(&Button {
            id: BELL.into(),
            name: Some("Bell".into()),
            sorting: 3,
            ..Button::default()
        })
        .unwrap();
    let mut board = Soundboard::from_store(store.clone());
    let main = iced::window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::BeginReorder(initial.id.clone()));
    let _ = board.update(Message::HoverReorder(BELL.into()));
    let _ = board.update(Message::EndReorder(main));
    let expected = board.buttons.clone();
    assert_eq!(
        expected.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        [HORN, BELL, initial.id.as_str()]
    );
    assert_eq!(board.default_button_count, 3);
    drop(board);
    let mut board = Soundboard::from_store(store.clone());
    assert_eq!(board.buttons, expected);
    let added = uuid::Uuid::now_v7().to_string();
    board.choosing_file = Some(added.clone());
    let _ = board.update(Message::AudioChosen {
        id: added.clone(),
        path: Some(dir.path().join("new.wav")),
    });
    assert_eq!(board.buttons.last().unwrap().id, added);
    assert!(board.button_categories.contains_key(&added));
    let expected = board.buttons.clone();
    drop(board);
    assert_eq!(Soundboard::from_store(store).buttons, expected);
}

#[test]
fn mouse3_named_category_reorder_keeps_default_category_untouched() {
    let dir = directory();
    let store = Store::new(dir.path());
    store.load_or_create().unwrap();
    let before = fs::read(dir.path().join("categories/default.json")).unwrap();
    let named = Category {
        category_name: Some("Effects".into()),
        entries: vec![
            Button {
                id: HORN.into(),
                sorting: 1,
                ..Button::default()
            },
            Button {
                id: BELL.into(),
                sorting: 2,
                ..Button::default()
            },
        ],
        ..Category::default()
    };
    fs::write(
        dir.path().join(format!("categories/{}.json", named.id)),
        serde_json::to_vec_pretty(&named).unwrap(),
    )
    .unwrap();
    let mut board = Soundboard::from_store(store.clone());
    let main = iced::window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::BeginReorder(BELL.into()));
    let _ = board.update(Message::HoverReorder(HORN.into()));
    let _ = board.update(Message::EndReorder(main));
    assert_eq!(board.default_button_count, 1);
    assert_eq!(board.buttons[1].id, BELL);
    let expected = board.buttons.clone();
    drop(board);
    assert_eq!(store.load_or_create().unwrap().ordered_buttons(), expected);
    assert_eq!(
        fs::read(dir.path().join("categories/default.json")).unwrap(),
        before
    );
}

#[test]
fn overlap_mode_can_be_enabled_and_disabled_and_survives_reopening() {
    let dir = directory();
    let store = Store::new(dir.path());
    let mut board = Soundboard::from_store(store.clone());
    let id = board.buttons[0].id.clone();
    assert!(!board.buttons[0].overlap);
    let _ = board.update(Message::OpenButtonOptions(id.clone()));
    let generation = board.options.generation;
    let _ = board.update(Message::OptionsChanged {
        generation,
        action: button_options::Action::Overlap(true),
    });
    drop(board);
    let mut board = Soundboard::from_store(store.clone());
    assert!(board.buttons[0].overlap);
    let _ = board.update(Message::OpenButtonOptions(id));
    let generation = board.options.generation;
    let _ = board.update(Message::OptionsChanged {
        generation,
        action: button_options::Action::Overlap(false),
    });
    drop(board);
    assert!(!Soundboard::from_store(store).buttons[0].overlap);
}
