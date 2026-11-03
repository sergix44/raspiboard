use super::super::*;
use raspiboard::storage::{Category, State};

fn board() -> Soundboard {
    Soundboard::from_saved(
        State {
            categories: vec![Category {
                entries: vec![
                    Button {
                        id: "horn".into(),
                        name: Some("Brass HORN".into()),
                        full_path: Some("/sounds/trumpet.wav".into()),
                        ..Button::default()
                    },
                    Button {
                        id: "bell".into(),
                        full_path: Some("/sounds/Door Bell.ogg".into()),
                        ..Button::default()
                    },
                    Button {
                        id: "horn2".into(),
                        name: Some("Air horn".into()),
                        ..Button::default()
                    },
                    Button {
                        id: "empty".into(),
                        ..Button::default()
                    },
                ],
                ..Category::default()
            }],
            ..State::default()
        },
        None,
    )
}

fn visible(board: &Soundboard) -> Vec<&str> {
    board.visible_buttons().map(|b| b.id.as_str()).collect()
}

#[test]
fn search_matches_displayed_names_case_insensitively_and_trims_whitespace() {
    let mut board = board();
    for (query, expected) in [
        ("  hOrN  ", vec!["horn", "horn2"]),
        ("bell", vec!["bell"]),
        ("trumpet", vec![]),
        ("choose sound", vec!["empty"]),
        ("not found", vec![]),
        ("   ", vec!["horn", "bell", "horn2", "empty"]),
        ("", vec!["horn", "bell", "horn2", "empty"]),
    ] {
        let _ = board.update(Message::SearchChanged(query.into()));
        assert_eq!(visible(&board), expected, "query: {query:?}");
    }
}

#[test]
fn search_cancels_transient_grid_actions_without_mutating_the_board_or_audio() {
    let mut board = board();
    let original = board.buttons.clone();
    let active = board.active_button.clone();
    let hotkeys = board.hotkeys.config.revision;
    board.audio.playback = PlaybackStatus::Playing;
    board.grid_menu = Some(Point::ORIGIN);
    board.button_menu = Some("horn".into());
    board.reorder = Some(reorder::Drag {
        source: "horn".into(),
        target: Some("bell".into()),
    });
    let _ = board.update(Message::SearchChanged("bell".into()));
    assert!(board.grid_menu.is_none());
    assert!(board.button_menu.is_none());
    assert!(board.reorder.is_none());
    assert_eq!(board.buttons, original);
    assert_eq!(board.active_button, active);
    assert_eq!(board.audio.playback, PlaybackStatus::Playing);
    assert_eq!(board.hotkeys.config.revision, hotkeys);
    assert_eq!(board.last_saved_revision, 0);
    assert!(board.play_command("horn").is_none());
    board.audio.connected = true;
    assert!(matches!(
        board.play_command("horn"),
        Some(Command::Play { .. })
    ));
}

#[test]
fn search_reordering_keeps_hidden_pads_and_clearing_reveals_updated_order() {
    let mut board = board();
    let main = window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::SearchChanged("horn".into()));
    let _ = board.update(Message::BeginReorder("horn".into()));
    let _ = board.update(Message::HoverReorder("horn2".into()));
    let _ = board.update(Message::EndReorder(main));
    assert_eq!(visible(&board), ["horn2", "horn"]);
    let _ = board.update(Message::SearchChanged(String::new()));
    assert_eq!(visible(&board), ["bell", "horn2", "horn", "empty"]);
    assert_eq!(
        board.buttons.iter().map(|b| b.sorting).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
}
