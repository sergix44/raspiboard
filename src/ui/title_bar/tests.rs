use super::*;
use crate::ui::{Soundboard, test_renderer::TestRenderer};
use iced::advanced::{Layout, Shell, clipboard, layout, widget::Tree};
use iced::{Event, Point, Rectangle, Size};

fn event(
    widget: &mut Element<'_, Message, Theme, TestRenderer>,
    tree: &mut Tree,
    event: Event,
    point: Point,
    size: Size,
) -> Vec<Message> {
    let node = widget
        .as_widget_mut()
        .layout(tree, &TestRenderer, &layout::Limits::new(size, size));
    let mut messages = Vec::new();
    widget.as_widget_mut().update(
        tree,
        &event,
        Layout::new(&node),
        mouse::Cursor::Available(point),
        &TestRenderer,
        &mut clipboard::Null,
        &mut Shell::new(&mut messages),
        &Rectangle::with_size(size),
    );
    messages
}

fn press(
    widget: &mut Element<'_, Message, Theme, TestRenderer>,
    tree: &mut Tree,
    point: Point,
    size: Size,
) -> Vec<Message> {
    event(
        widget,
        tree,
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        point,
        size,
    )
}

#[test]
fn title_bar_blank_space_moves_and_double_click_maximizes() {
    let mut widget = view::<TestRenderer>("", false);
    let mut tree = Tree::new(widget.as_widget());
    let size = Size::new(1000.0, HEIGHT);
    let point = Point::new(300.0, HEIGHT / 2.0);
    let first = press(&mut widget, &mut tree, point, size);
    assert!(first.iter().any(|m| matches!(m, Message::DragMainWindow)));
    let _ = event(
        &mut widget,
        &mut tree,
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        point,
        size,
    );
    let second = press(&mut widget, &mut tree, point, size);
    assert!(second.iter().any(|m| matches!(m, Message::ToggleMaximize)));
}

#[test]
fn title_bar_search_clear_and_window_buttons_never_drag() {
    for (x, expected) in [
        (500.0, "search"),
        (635.0, "clear"),
        (888.0, "minimize"),
        (930.0, "maximize"),
        (972.0, "close"),
    ] {
        let mut widget = view::<TestRenderer>("horn", false);
        let mut tree = Tree::new(widget.as_widget());
        let size = Size::new(1000.0, HEIGHT);
        let point = Point::new(x, HEIGHT / 2.0);
        let mut messages = press(&mut widget, &mut tree, point, size);
        messages.extend(event(
            &mut widget,
            &mut tree,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            point,
            size,
        ));
        assert!(
            !messages.iter().any(|m| matches!(
                m,
                Message::DragMainWindow | Message::ToggleMaximize
            ) && expected != "maximize"),
            "{expected}: {messages:?}"
        );
        match expected {
            "search" => assert!(messages.is_empty()),
            "clear" => assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::SearchChanged(q) if q.is_empty()))
            ),
            "minimize" => assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::MinimizeMainWindow))
            ),
            "maximize" => assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::ToggleMaximize))
            ),
            "close" => assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::CloseMainWindow))
            ),
            _ => unreachable!(),
        }
        assert!(
            !messages
                .iter()
                .any(|m| matches!(m, Message::DragMainWindow)),
            "{expected}"
        );
    }
}

#[test]
fn title_bar_raised_controls_capture_their_top_caps_and_cancel_outside_release() {
    for (x, expected) in [(888.0, "minimize"), (930.0, "maximize"), (972.0, "close")] {
        for outside in [false, true] {
            let mut widget = view::<TestRenderer>("", false);
            let mut tree = Tree::new(widget.as_widget());
            let size = Size::new(1000.0, HEIGHT);
            let point = Point::new(x, 1.0);
            let mut messages = press(&mut widget, &mut tree, point, size);
            assert!(
                !messages
                    .iter()
                    .any(|m| matches!(m, Message::DragMainWindow)),
                "{expected}: top cap starts a window drag"
            );
            messages.extend(event(
                &mut widget,
                &mut tree,
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                if outside {
                    Point::new(300.0, 22.0)
                } else {
                    point
                },
                size,
            ));
            let activated = messages.iter().any(|m| match expected {
                "minimize" => matches!(m, Message::MinimizeMainWindow),
                "maximize" => matches!(m, Message::ToggleMaximize),
                "close" => matches!(m, Message::CloseMainWindow),
                _ => unreachable!(),
            });
            assert_eq!(activated, !outside, "{expected}: {messages:?}");
        }
    }
}

#[test]
fn title_bar_resize_frame_covers_all_directions_but_not_content_or_maximized_window() {
    let size = Size::new(1000.0, 680.0);
    for (point, expected) in [
        (Point::new(2.0, 2.0), "NorthWest"),
        (Point::new(500.0, 2.0), "North"),
        (Point::new(998.0, 2.0), "NorthEast"),
        (Point::new(2.0, 340.0), "West"),
        (Point::new(998.0, 340.0), "East"),
        (Point::new(2.0, 678.0), "SouthWest"),
        (Point::new(500.0, 678.0), "South"),
        (Point::new(998.0, 678.0), "SouthEast"),
        (Point::new(500.0, 340.0), "None"),
    ] {
        for maximized in [false, true] {
            let mut widget =
                frame::<TestRenderer>(space().width(Fill).height(Fill).into(), maximized);
            let mut tree = Tree::new(widget.as_widget());
            let messages = press(&mut widget, &mut tree, point, size);
            let direction = messages
                .iter()
                .find_map(|m| match m {
                    Message::ResizeMainWindow(direction) => Some(format!("{direction:?}")),
                    _ => None,
                })
                .unwrap_or("None".into());
            assert_eq!(direction, if maximized { "None" } else { expected });
        }
    }
}

#[test]
fn title_bar_maximize_state_ignores_closed_windows_and_resets_on_reopen() {
    let mut board = Soundboard::default();
    let old = window::Id::unique();
    let main = window::Id::unique();
    board.main_window = Some(main);
    let _ = board.update(Message::MainWindowMaximized(old, true));
    assert!(!board.main_maximized);
    let _ = board.update(Message::MainWindowMaximized(main, true));
    assert!(board.main_maximized);
    board.main_window = None;
    let _ = board.open_main();
    assert!(!board.main_maximized);
}

#[test]
fn title_bar_decorations_change_only_for_the_main_window() {
    assert!(crate::ui::windows::window_settings().decorations);
    assert!(!crate::ui::windows::window_settings().transparent);
    let main = crate::ui::windows::main_window_settings();
    assert!(!main.decorations);
    assert!(
        main.transparent,
        "rounded corners require an alpha-capable window"
    );
    assert!(!main.exit_on_close_request);
    assert_eq!(main.min_size, Some(Size::new(1000.0, 360.0)));
}
