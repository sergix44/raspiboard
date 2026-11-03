use super::*;
use crate::ui::test_renderer::TestRenderer;
use iced::advanced::{Layout, Shell, clipboard, layout, widget::Tree};
use iced::{Event, Point, Rectangle, Size, keyboard, mouse};

#[test]
fn hover_wheel_adjusts_gain_once_with_or_without_control_and_clamps() {
    for (initial, y, expected) in [
        (-5.5, 1.0, -5.0),
        (-5.5, -1.0, -6.0),
        (15.0, 1.0, 15.0),
        (-15.0, -1.0, -15.0),
    ] {
        for control in [false, true] {
            let mut input = view::<f32, TestRenderer>(initial, |value| value);
            let mut tree = Tree::new(input.as_widget());
            let node = input.as_widget_mut().layout(
                &mut tree,
                &TestRenderer,
                &layout::Limits::new(Size::ZERO, Size::new(240.0, 22.0)),
            );
            let mut messages = Vec::new();
            if control {
                input.as_widget_mut().update(
                    &mut tree,
                    &Event::Keyboard(keyboard::Event::ModifiersChanged(keyboard::Modifiers::CTRL)),
                    Layout::new(&node),
                    mouse::Cursor::Available(Point::new(100.0, 10.0)),
                    &TestRenderer,
                    &mut clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &Rectangle::with_size(Size::new(240.0, 22.0)),
                );
            }
            let mut shell = Shell::new(&mut messages);
            input.as_widget_mut().update(
                &mut tree,
                &Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y },
                }),
                Layout::new(&node),
                mouse::Cursor::Available(Point::new(100.0, 10.0)),
                &TestRenderer,
                &mut clipboard::Null,
                &mut shell,
                &Rectangle::with_size(Size::new(240.0, 22.0)),
            );
            assert!(shell.is_event_captured());
            assert_eq!(messages.last().copied().unwrap_or(initial), expected);
            assert!(messages.len() <= 1);
        }
    }
}
