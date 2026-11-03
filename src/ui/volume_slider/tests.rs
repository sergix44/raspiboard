use super::*;
use crate::ui::test_renderer::TestRenderer;
use iced::advanced::{Layout, Shell, clipboard, layout, mouse, widget::Tree};
use iced::{Event, Point, Rectangle, Size};
use raspiboard::theme::ThemeChoice;

fn background_colors(background: iced::Background) -> Vec<iced::Color> {
    match background {
        iced::Background::Color(color) => vec![color],
        iced::Background::Gradient(iced::gradient::Gradient::Linear(gradient)) => gradient
            .stops
            .into_iter()
            .flatten()
            .map(|stop| stop.color)
            .collect(),
    }
}

#[test]
fn slider_has_a_neutral_hardware_handle_separate_from_its_colored_fill() {
    for choice in ThemeChoice::ALL {
        let theme = choice.iced();
        for status in [
            slider::Status::Active,
            slider::Status::Hovered,
            slider::Status::Dragged,
        ] {
            let appearance = style(&theme, status);
            for color in background_colors(appearance.handle.background) {
                assert!(
                    color.r.max(color.g).max(color.b) - color.r.min(color.g).min(color.b) < 0.1
                );
            }
            assert_ne!(appearance.rail.backgrounds.0, appearance.rail.backgrounds.1);
        }
    }
}

#[test]
fn console_handle_has_a_stronger_metallic_bevel_than_deck() {
    let contrast = |choice: ThemeChoice| {
        let appearance = style(&choice.iced(), slider::Status::Active);
        let colors = background_colors(appearance.handle.background);
        let lightest = colors.iter().map(|color| color.r).fold(0.0, f32::max);
        let darkest = colors.iter().map(|color| color.r).fold(1.0, f32::min);
        lightest - darkest
    };
    assert!(contrast(ThemeChoice::Console) > contrast(ThemeChoice::Deck));
}

#[test]
fn disabled_slider_ignores_pointer_wheel_and_keyboard_events() {
    for enabled in [false, true] {
        let mut widget = view::<u8, TestRenderer>(50, |volume| volume, enabled);
        let mut tree = Tree::new(widget.as_widget());
        let layout = widget.as_widget_mut().layout(
            &mut tree,
            &TestRenderer,
            &layout::Limits::new(Size::ZERO, Size::new(160.0, 22.0)),
        );
        let mut messages = Vec::new();
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(150.0, 10.0),
            }),
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
                iced::keyboard::Modifiers::CTRL,
            )),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
                physical_key: iced::keyboard::key::Physical::Code(
                    iced::keyboard::key::Code::ArrowUp,
                ),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                repeat: false,
            }),
        ] {
            widget.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&layout),
                mouse::Cursor::Available(Point::new(140.0, 10.0)),
                &TestRenderer,
                &mut clipboard::Null,
                &mut Shell::new(&mut messages),
                &Rectangle::with_size(Size::new(160.0, 22.0)),
            );
        }
        assert_eq!(!messages.is_empty(), enabled);
    }
}
