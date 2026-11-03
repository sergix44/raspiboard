use super::*;
use crate::ui::Soundboard;
use crate::ui::test_renderer::TestRenderer;
use raspiboard::storage::{Button, Category, State};

fn board() -> Soundboard {
    let entries = (1..=3)
        .map(|sorting| Button {
            sorting,
            name: Some(format!("Pad {sorting}")),
            ..Button::default()
        })
        .collect();
    let mut board = Soundboard::from_saved(
        State {
            categories: vec![Category {
                entries,
                ..Category::default()
            }],
            ..State::default()
        },
        None,
    );
    board.main_window = Some(iced::window::Id::unique());
    board
}

#[test]
fn release_moves_to_target_position_without_changing_audio_or_active_pad() {
    for (from, to) in [(0, 2), (2, 0)] {
        let mut board = board();
        let original = board.buttons.clone();
        let source = original[from].id.clone();
        let target = original[to].id.clone();
        let active = board.active_button.clone();
        board.audio.playback = raspiboard::audio::PlaybackStatus::Playing;
        let _ = board.update(Message::BeginReorder(source.clone()));
        let _ = board.update(Message::HoverReorder(target));
        assert_eq!(board.buttons, original, "hover must not change order");
        let _ = board.update(Message::EndReorder(board.main_window.unwrap()));
        assert_eq!(board.buttons[to].id, source);
        assert_eq!(board.active_button, active);
        assert_eq!(
            board.audio.playback,
            raspiboard::audio::PlaybackStatus::Playing
        );
        assert!(board.reorder.is_none());
        assert_eq!(
            board.buttons.iter().map(|b| b.sorting).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        for button in &board.buttons {
            let mut actual = button.clone();
            let expected = original.iter().find(|b| b.id == actual.id).unwrap();
            actual.sorting = expected.sorting;
            assert_eq!(&actual, expected);
        }
    }
}

#[test]
fn leaving_drop_target_or_canceling_does_not_change_order() {
    let mut board = board();
    let original = board.buttons.clone();
    let _ = board.update(Message::BeginReorder(original[0].id.clone()));
    let _ = board.update(Message::HoverReorder(original[2].id.clone()));
    let _ = board.update(Message::LeaveReorder(original[2].id.clone()));
    let _ = board.update(Message::EndReorder(board.main_window.unwrap()));
    assert_eq!(board.buttons, original);
    let _ = board.update(Message::BeginReorder(original[0].id.clone()));
    let _ = board.update(Message::HoverReorder(original[2].id.clone()));
    let _ = board.update(Message::EscapePressed(board.main_window.unwrap()));
    assert!(board.reorder.is_none());
    let _ = board.update(Message::EndReorder(board.main_window.unwrap()));
    assert_eq!(board.buttons, original);
}

#[test]
fn dropping_across_categories_is_rejected_without_moving_membership() {
    let mut board = board();
    let original = board.buttons.clone();
    board
        .button_categories
        .insert(original[2].id.clone(), uuid::Uuid::now_v7().to_string());
    let _ = board.update(Message::BeginReorder(original[0].id.clone()));
    let _ = board.update(Message::HoverReorder(original[2].id.clone()));
    let _ = board.update(Message::EndReorder(board.main_window.unwrap()));
    assert_eq!(board.buttons, original);
}

fn native_pad(edge: Option<bool>) -> Element<'static, Message, Theme, TestRenderer> {
    use iced::widget::button;
    pad(
        button(space()).width(100).height(50).into(),
        "pad",
        false,
        edge,
    )
}

fn native_event(
    widget: &mut Element<'_, Message, Theme, TestRenderer>,
    tree: &mut iced::advanced::widget::Tree,
    event: Event,
    cursor: mouse::Cursor,
) -> Vec<Message> {
    use iced::advanced::{Layout, Shell, clipboard, layout};
    let layout = widget.as_widget_mut().layout(
        tree,
        &TestRenderer,
        &layout::Limits::new(iced::Size::ZERO, iced::Size::new(100.0, 50.0)),
    );
    let mut messages = Vec::new();
    widget.as_widget_mut().update(
        tree,
        &event,
        Layout::new(&layout),
        cursor,
        &TestRenderer,
        &mut clipboard::Null,
        &mut Shell::new(&mut messages),
        &iced::Rectangle::with_size(iced::Size::new(100.0, 50.0)),
    );
    messages
}

#[test]
fn insertion_marker_rebuild_preserves_pointer_exit_tracking() {
    let mut widget = native_pad(None);
    let mut tree = iced::advanced::widget::Tree::new(widget.as_widget());
    let inside = iced::Point::new(50.0, 25.0);
    let _ = native_event(
        &mut widget,
        &mut tree,
        Event::Mouse(mouse::Event::CursorMoved { position: inside }),
        mouse::Cursor::Available(inside),
    );
    widget = native_pad(Some(true));
    tree.diff(widget.as_widget());
    let outside = iced::Point::new(150.0, 25.0);
    let messages = native_event(
        &mut widget,
        &mut tree,
        Event::Mouse(mouse::Event::CursorMoved { position: outside }),
        mouse::Cursor::Available(outside),
    );
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::LeaveReorder(id) if id == "pad"))
    );
}

#[test]
fn disabled_playback_pad_still_accepts_middle_button_drag_without_playing() {
    let mut widget = native_pad(None);
    let mut tree = iced::advanced::widget::Tree::new(widget.as_widget());
    let messages = native_event(
        &mut widget,
        &mut tree,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
        mouse::Cursor::Available(iced::Point::new(50.0, 25.0)),
    );
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::BeginReorder(id) if id == "pad"))
    );
    assert!(!messages.iter().any(|m| matches!(m, Message::PlayButton(_))));
}

#[test]
fn global_release_and_focus_events_complete_or_cancel_even_outside_a_pad() {
    let mut board = board();
    let main = board.main_window.unwrap();
    let original = board.buttons.clone();
    let _ = board.update(Message::BeginReorder(original[0].id.clone()));
    let _ = board.update(Message::HoverReorder(original[2].id.clone()));
    let cancel = window_event(&Event::Window(window::Event::Unfocused), main).unwrap();
    let _ = board.update(cancel);
    assert!(board.reorder.is_none());
    let release = window_event(
        &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)),
        main,
    )
    .unwrap();
    let _ = board.update(release);
    assert_eq!(board.buttons, original);
    let _ = board.update(Message::BeginReorder(original[0].id.clone()));
    let _ = board.update(Message::HoverReorder(original[2].id.clone()));
    let leave = window_event(&Event::Mouse(mouse::Event::CursorLeft), main).unwrap();
    let _ = board.update(leave);
    assert!(board.reorder.is_none());
    assert_eq!(board.buttons, original);
}

#[test]
fn stale_hover_and_deleted_source_never_recreate_a_pad() {
    let mut board = board();
    let source = board.buttons[0].id.clone();
    let _ = board.update(Message::BeginReorder(source.clone()));
    let _ = board.update(Message::HoverReorder(board.buttons[2].id.clone()));
    let _ = board.update(Message::DeleteButton(source.clone()));
    assert!(board.reorder.is_none());
    let _ = board.update(Message::HoverReorder(source.clone()));
    let _ = board.update(Message::EndReorder(board.main_window.unwrap()));
    assert!(!board.buttons.iter().any(|b| b.id == source));
    let current = board.buttons.clone();
    board.choosing_file = Some("pending".into());
    let _ = board.update(Message::BeginReorder(board.buttons[0].id.clone()));
    assert!(board.reorder.is_none());
    assert_eq!(board.buttons, current);
}
