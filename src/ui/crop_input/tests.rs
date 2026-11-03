use super::*;
use crate::ui::test_renderer::TestRenderer;
use iced::advanced::{Layout, Shell, clipboard, layout, widget::Tree};
use iced::{Event, Point, Rectangle, Size, keyboard, mouse};

fn scroll(
    draft: &str,
    default: u64,
    maximum: Option<u64>,
    delta: mouse::ScrollDelta,
    hovered: bool,
) -> (Vec<String>, bool) {
    let mut input = view::<String, TestRenderer>(draft, default, maximum, |value| value);
    let mut tree = Tree::new(input.as_widget());
    let node = input.as_widget_mut().layout(
        &mut tree,
        &TestRenderer,
        &layout::Limits::new(Size::ZERO, Size::new(240.0, 60.0)),
    );
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    input.as_widget_mut().update(
        &mut tree,
        &Event::Mouse(mouse::Event::WheelScrolled { delta }),
        Layout::new(&node),
        if hovered {
            mouse::Cursor::Available(Point::new(30.0, 15.0))
        } else {
            mouse::Cursor::Unavailable
        },
        &TestRenderer,
        &mut clipboard::Null,
        &mut shell,
        &Rectangle::with_size(Size::new(240.0, 60.0)),
    );
    let captured = shell.is_event_captured();
    (messages, captured)
}

#[test]
fn hovering_crop_inputs_steps_exact_milliseconds_and_captures_the_wheel() {
    for delta in [
        mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        mouse::ScrollDelta::Pixels { x: 0.0, y: 20.0 },
    ] {
        assert_eq!(
            scroll("0.143", 0, Some(1077), delta, true),
            (vec!["0.153".into()], true)
        );
        assert_eq!(
            scroll("0.143", 0, Some(1077), delta, false),
            (vec![], false)
        );
    }
}

#[test]
fn crop_wheel_clamps_at_file_bounds_and_keeps_full_source_defaults() {
    assert_eq!(
        scroll(
            "0.005",
            0,
            Some(1000),
            mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            true
        )
        .0,
        [""]
    );
    assert_eq!(
        scroll(
            "0.995",
            1000,
            Some(1000),
            mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            true
        )
        .0,
        [""]
    );
    assert_eq!(
        scroll(
            "",
            1000,
            Some(1000),
            mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            true
        )
        .0,
        ["0.99"]
    );
    assert!(
        scroll(
            "0.143",
            0,
            Some(1000),
            mouse::ScrollDelta::Lines { x: 1.0, y: 0.0 },
            true
        )
        .0
        .is_empty()
    );
}

struct Clipboard(String);

impl iced::advanced::Clipboard for Clipboard {
    fn read(&self, _: clipboard::Kind) -> Option<String> {
        Some(self.0.clone())
    }
    fn write(&mut self, _: clipboard::Kind, contents: String) {
        self.0 = contents;
    }
}

fn paste(value: &str) -> Vec<String> {
    let mut input = view::<String, TestRenderer>("0.143", 0, Some(5000), |value| value);
    let mut tree = Tree::new(input.as_widget());
    let node = input.as_widget_mut().layout(
        &mut tree,
        &TestRenderer,
        &layout::Limits::new(Size::ZERO, Size::new(240.0, 60.0)),
    );
    let mut messages = Vec::new();
    let mut clipboard = Clipboard(value.into());
    let key = |character: &str| {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(character.into()),
            modified_key: keyboard::Key::Character(character.into()),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyV),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        })
    };
    for event in [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Event::Keyboard(keyboard::Event::ModifiersChanged(keyboard::Modifiers::CTRL)),
        key("a"),
        key("v"),
    ] {
        input.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(30.0, 15.0)),
            &TestRenderer,
            &mut clipboard,
            &mut Shell::new(&mut messages),
            &Rectangle::with_size(Size::new(240.0, 60.0)),
        );
    }
    messages
}

#[test]
fn numeric_crop_inputs_accept_decimal_seconds_and_reject_non_numeric_or_out_of_range_paste() {
    assert_eq!(paste("1.250"), ["1.25"]);
    assert_eq!(paste("1.23456"), ["1.235"]);
    for invalid in ["words", "-1", "6", "NaN", "inf"] {
        assert!(paste(invalid).is_empty(), "accepted {invalid}");
    }
}

fn edit_keys(default_ms: u64, keys: &[keyboard::Key]) -> Vec<String> {
    let mut input = view::<String, TestRenderer>("0.143", default_ms, Some(1000), |value| value);
    let mut tree = Tree::new(input.as_widget());
    let limits = layout::Limits::new(Size::ZERO, Size::new(240.0, 60.0));
    let mut node = input
        .as_widget_mut()
        .layout(&mut tree, &TestRenderer, &limits);
    let mut messages = Vec::new();
    let key_event = |key: keyboard::Key, modifiers| {
        Event::Keyboard(keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            text: match &key {
                keyboard::Key::Character(text) if modifiers == keyboard::Modifiers::empty() => {
                    Some(text.clone())
                }
                _ => None,
            },
            key,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyA),
            location: keyboard::Location::Standard,
            modifiers,
            repeat: false,
        })
    };
    let setup = [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Event::Keyboard(keyboard::Event::ModifiersChanged(keyboard::Modifiers::CTRL)),
        key_event(
            keyboard::Key::Character("a".into()),
            keyboard::Modifiers::CTRL,
        ),
        Event::Keyboard(keyboard::Event::ModifiersChanged(
            keyboard::Modifiers::empty(),
        )),
    ];
    for event in setup.into_iter().chain(
        keys.iter()
            .cloned()
            .map(|key| key_event(key, keyboard::Modifiers::empty())),
    ) {
        let mut published = Vec::new();
        input.as_widget_mut().update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(30.0, 15.0)),
            &TestRenderer,
            &mut clipboard::Null,
            &mut Shell::new(&mut published),
            &Rectangle::with_size(Size::new(240.0, 60.0)),
        );
        if let Some(draft) = published.last() {
            input = view::<String, TestRenderer>(draft, default_ms, Some(1000), |value| value);
            tree.diff(input.as_widget());
            node = input
                .as_widget_mut()
                .layout(&mut tree, &TestRenderer, &limits);
        }
        messages.extend(published);
    }
    messages
}

#[test]
fn fractional_end_can_be_typed_through_zero_without_resetting_to_duration() {
    assert_eq!(
        scroll(
            "0.005",
            1000,
            Some(1000),
            mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            true
        )
        .0,
        ["0"]
    );
    let keys: Vec<_> = ["0", ".", "5"]
        .map(|key| keyboard::Key::Character(key.into()))
        .into();
    assert_eq!(edit_keys(1000, &keys), ["0", "0.5"]);
}

#[test]
fn clearing_either_numeric_field_resets_only_that_boundary_immediately() {
    for default in [0, 1000] {
        assert_eq!(
            edit_keys(
                default,
                &[keyboard::Key::Named(keyboard::key::Named::Backspace)]
            ),
            [""]
        );
    }
}
