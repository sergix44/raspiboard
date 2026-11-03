use super::*;
use crate::ui::test_renderer::TestRenderer;
use iced::Point;
use iced::advanced::clipboard;
use iced::widget::{button, mouse_area, space};
use raspiboard::theme::ThemeChoice;

#[test]
fn passthrough_blinks_every_second_with_a_brief_filament_fade() {
    let level = |ms| blink_intensity(Duration::from_millis(ms));
    assert_eq!(level(0), 0.0);
    assert!(level(20) > 0.0 && level(20) < 1.0);
    assert_eq!(level(40), 1.0);
    assert_eq!(level(999), 1.0);
    assert_eq!(level(500), 1.0);
    assert_eq!(level(1000), 1.0);
    assert!(level(1030) > 0.0 && level(1030) < 1.0);
    assert_eq!(level(1060), 0.0);
    assert_eq!(level(1999), 0.0);
    assert_eq!(level(2000), 0.0);
    assert_eq!(level(2040), 1.0);
}

#[test]
fn passthrough_blink_requests_redraws_only_while_active() {
    let make = |active| {
        blinking::<(), TestRenderer>(
            button(space()).width(32).height(28).on_press(()),
            true,
            active,
            Color::from_rgb8(0xa7, 0xf3, 0xd0),
        )
    };
    let mut widget = make(true);
    let mut tree = Tree::new(widget.as_widget());
    let node = widget.as_widget_mut().layout(
        &mut tree,
        &TestRenderer,
        &layout::Limits::new(Size::ZERO, Size::new(32.0, 28.0)),
    );
    let now = Instant::now();
    for active in [true, false] {
        widget = make(active);
        tree.diff(widget.as_widget());
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        widget.as_widget_mut().update(
            &mut tree,
            &Event::Window(window::Event::RedrawRequested(now)),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &TestRenderer,
            &mut clipboard::Null,
            &mut shell,
            &Rectangle::with_size(Size::new(32.0, 28.0)),
        );
        assert_eq!(
            shell.redraw_request() != window::RedrawRequest::Wait,
            active
        );
    }
}

#[test]
fn custom_colors_parse_only_full_rgb_and_round_trip_opaquely() {
    for (input, expected) in [
        ("#000000", Color::BLACK),
        ("#FFFFFF", Color::WHITE),
        ("#12abEF", Color::from_rgb8(0x12, 0xab, 0xef)),
    ] {
        assert_eq!(parse_color(input), Some(expected));
        assert_eq!(parse_color(&color_hex(expected)), Some(expected));
    }
    assert_eq!(
        color_hex(Color::from_rgba8(0x12, 0xab, 0xef, 0.2)),
        "#12ABEF"
    );
    for invalid in [
        "",
        "#abc",
        "123456",
        "#12345678",
        "#12345g",
        " #123456",
        "#123456 ",
        "#é1234",
    ] {
        assert_eq!(parse_color(invalid), None, "accepted {invalid:?}");
    }
}

#[test]
fn idle_default_faces_are_neutral_and_readable_instead_of_accent_filled() {
    for choice in ThemeChoice::ALL {
        let theme = choice.iced();
        for enabled in [false, true] {
            let idle = colors(&theme, enabled, false, Tone::Accent);
            let face = idle.color;
            assert!(face.r.max(face.g).max(face.b) < 0.3);
            assert!(face.r.max(face.g).max(face.b) - face.r.min(face.g).min(face.b) < 0.1);
            assert!(idle.text.is_readable_on(face));
        }
    }
}

#[derive(Default)]
struct Surfaces(Vec<(renderer::Quad, iced::Background)>);

impl renderer::Renderer for Surfaces {
    fn start_layer(&mut self, _bounds: Rectangle) {}
    fn end_layer(&mut self) {}
    fn start_transformation(&mut self, _transformation: iced::Transformation) {}
    fn end_transformation(&mut self) {}
    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<iced::Background>) {
        self.0.push((quad, background.into()));
    }
    fn reset(&mut self, _new_bounds: Rectangle) {
        self.0.clear();
    }
    fn allocate_image(
        &mut self,
        handle: &iced::advanced::image::Handle,
        callback: impl FnOnce(Result<iced::advanced::image::Allocation, iced::advanced::image::Error>)
        + Send
        + 'static,
    ) {
        renderer::Renderer::allocate_image(&mut TestRenderer, handle, callback);
    }
}

fn drawn_face(
    choice: ThemeChoice,
    playing: bool,
    tone: Tone,
) -> (renderer::Quad, iced::Background) {
    let mut widget = raised::<(), Surfaces>(
        button(space()).width(180).height(86).on_press(()),
        true,
        playing,
        tone,
        false,
    );
    let mut tree = Tree::new(widget.as_widget());
    let mut surfaces = Surfaces::default();
    let node = widget.as_widget_mut().layout(
        &mut tree,
        &surfaces,
        &layout::Limits::new(Size::ZERO, Size::new(180.0, 86.0)),
    );
    widget.as_widget().draw(
        &tree,
        &mut surfaces,
        &choice.iced(),
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::with_size(Size::new(180.0, 86.0)),
    );
    *surfaces.0.last().unwrap()
}

#[test]
fn faces_have_crisp_edges_and_console_has_a_stronger_bevel() {
    let mut ranges = Vec::new();
    for choice in ThemeChoice::ALL {
        let (quad, background) = drawn_face(*choice, false, Tone::Accent);
        assert_eq!(quad.border.width, 1.0);
        let iced::Background::Gradient(iced::gradient::Gradient::Linear(gradient)) = background
        else {
            panic!("pad face must have a hardware bevel");
        };
        let colors: Vec<_> = gradient
            .stops
            .into_iter()
            .flatten()
            .map(|stop| stop.color)
            .collect();
        let lightest = colors.iter().map(|color| color.r).fold(0.0, f32::max);
        let darkest = colors.iter().map(|color| color.r).fold(1.0, f32::min);
        ranges.push(lightest - darkest);
    }
    assert!(ranges[1] > ranges[0]);
}

#[test]
fn playing_faces_use_the_same_cyan_for_their_outline_and_backlight() {
    for choice in ThemeChoice::ALL {
        let (idle, _) = drawn_face(*choice, false, Tone::Accent);
        let (playing, _) = drawn_face(*choice, true, Tone::Accent);
        assert_ne!(idle.border.color, playing.border.color);
        assert_eq!(playing.border.color, iced::color!(0x24b9ed));
    }
}

#[test]
fn custom_faces_keep_their_rgb_fill_and_matching_active_outline_in_both_finishes() {
    let color = iced::color!(0xff8040);
    for choice in ThemeChoice::ALL {
        for playing in [false, true] {
            let (quad, background) = drawn_face(*choice, playing, Tone::Custom(color));
            assert_eq!(background, iced::Background::Color(color));
            if playing {
                assert_eq!(quad.border.color, color);
            }
        }
    }
}

#[test]
fn custom_faces_keep_their_color_and_choose_readable_text_in_every_theme() {
    for theme in ThemeChoice::ALL.iter().map(|choice| choice.iced()) {
        for color in [Color::BLACK, Color::WHITE, iced::color!(0xfacc15)] {
            for playing in [false, true] {
                let pair = colors(&theme, true, playing, Tone::Custom(color));
                assert_eq!(pair.color, color);
                assert!(pair.text.is_readable_on(pair.color));
                assert_eq!(
                    style(&theme, button::Status::Active, playing, Tone::Custom(color)).text_color,
                    pair.text
                );
            }
            let disabled = colors(&theme, false, false, Tone::Custom(color));
            assert_eq!(disabled.color, color.scale_alpha(0.4));
            assert!(
                style(&theme, button::Status::Disabled, false, Tone::Custom(color))
                    .text_color
                    .a
                    < 1.0
            );
        }
    }
}

#[test]
fn playback_glow_matches_custom_color_and_respects_disabled_and_idle_states() {
    let color = iced::color!(0xff8040);
    for theme in ThemeChoice::ALL.iter().map(|choice| choice.iced()) {
        assert_eq!(
            backlight(&theme, true, true, Tone::Custom(color)),
            Some(color)
        );
        assert_eq!(backlight(&theme, false, true, Tone::Custom(color)), None);
        assert_eq!(backlight(&theme, true, false, Tone::Custom(color)), None);
        assert_eq!(
            backlight(&theme, true, true, Tone::Accent),
            Some(theme.extended_palette().primary.base.color)
        );
        let idle = colors(&theme, true, false, Tone::Accent);
        let active = colors(&theme, true, true, Tone::Accent);
        assert_ne!(idle.color, active.color);
        assert!(active.color.b > active.color.r);
        assert!(active.color.g > active.color.r);
        assert!(active.text.is_readable_on(active.color));
    }
}

#[test]
fn transport_buttons_use_semantic_colors_and_remain_distinct_when_disabled() {
    for theme in ThemeChoice::ALL.iter().map(|choice| choice.iced()) {
        let play = colors(&theme, true, false, Tone::Play);
        let stop = colors(&theme, true, false, Tone::Stop);
        let pause = colors(&theme, true, false, Tone::Pause);
        assert_eq!(pause.color, iced::color!(0xfacc15));
        assert_eq!(pause.text, Color::BLACK);
        assert_eq!(play.color, theme.extended_palette().success.base.color);
        assert_eq!(stop.color, iced::color!(0x7dd3fc));
        assert_ne!(play.color, stop.color);
        let disabled_play = colors(&theme, false, false, Tone::Play);
        let disabled_stop = colors(&theme, false, false, Tone::Stop);
        assert_ne!(disabled_play.color, disabled_stop.color);
        assert!(disabled_play.color.a < play.color.a);
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Play,
    Assign,
}

struct Pad {
    widget: Element<'static, Action, Theme, TestRenderer>,
    tree: Tree,
    layout: layout::Node,
}

impl Pad {
    fn widget(enabled: bool) -> Element<'static, Action, Theme, TestRenderer> {
        let button = button(space())
            .width(180)
            .height(86)
            .on_press_maybe(enabled.then_some(Action::Play));
        mouse_area(animated(button, enabled, false))
            .on_right_press(Action::Assign)
            .into()
    }

    fn new(enabled: bool) -> Self {
        let mut widget = Self::widget(enabled);
        let mut tree = Tree::new(widget.as_widget());
        let layout = widget.as_widget_mut().layout(
            &mut tree,
            &TestRenderer,
            &layout::Limits::new(Size::ZERO, Size::new(180.0, 86.0)),
        );
        Self {
            widget,
            tree,
            layout,
        }
    }

    fn update(
        &mut self,
        event: Event,
        cursor: mouse::Cursor,
    ) -> (Vec<Action>, window::RedrawRequest) {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        self.widget.as_widget_mut().update(
            &mut self.tree,
            &event,
            Layout::new(&self.layout),
            cursor,
            &TestRenderer,
            &mut clipboard::Null,
            &mut shell,
            &Rectangle::with_size(Size::new(180.0, 86.0)),
        );
        let redraw = shell.redraw_request();
        (messages, redraw)
    }

    fn motion(&self) -> &State {
        self.tree.children[0].state.downcast_ref::<State>()
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.widget = Self::widget(enabled);
        self.tree.diff(self.widget.as_widget());
    }
}

fn inside() -> mouse::Cursor {
    mouse::Cursor::Available(Point::new(90.0, 43.0))
}

#[test]
fn pressing_snaps_down_quickly_without_playing_until_release() {
    let mut pad = Pad::new(true);
    let (messages, redraw) = pad.update(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        inside(),
    );
    assert!(messages.is_empty());
    assert_eq!(redraw, window::RedrawRequest::NextFrame);
    let pressed_at = pad.motion().now;
    let _ = pad.update(
        Event::Window(window::Event::RedrawRequested(
            pressed_at + Duration::from_millis(40),
        )),
        inside(),
    );
    assert!((pad.motion().offset() + 2.0).abs() < 0.001);

    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        inside(),
    );
    assert_eq!(messages, [Action::Play]);
}

#[test]
fn releasing_outside_cancels_playback_and_motion_settles_without_redraws() {
    let mut pad = Pad::new(true);
    let _ = pad.update(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        inside(),
    );
    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        mouse::Cursor::Unavailable,
    );
    assert!(messages.is_empty());
    let released_at = pad.motion().now;
    let (_, redraw) = pad.update(
        Event::Window(window::Event::RedrawRequested(
            released_at + Duration::from_secs(1),
        )),
        mouse::Cursor::Unavailable,
    );
    assert!((pad.motion().offset() + 4.0).abs() < 0.001);
    assert_eq!(redraw, window::RedrawRequest::Wait);
}

#[test]
fn disabled_pad_stays_still_but_right_click_still_assigns_a_sound() {
    let mut pad = Pad::new(false);
    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        inside(),
    );
    assert!(messages.is_empty());
    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        inside(),
    );
    assert!(messages.is_empty());
    assert_eq!(pad.motion().offset(), 0.0);
    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        inside(),
    );
    assert_eq!(messages, [Action::Assign]);
}

#[test]
fn losing_playback_availability_cancels_a_held_press_across_rebuilds() {
    let mut pad = Pad::new(true);
    let _ = pad.update(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        inside(),
    );
    pad.set_enabled(false);
    let _ = pad.update(
        Event::Window(window::Event::RedrawRequested(Instant::now())),
        inside(),
    );
    pad.set_enabled(true);
    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        inside(),
    );
    assert!(
        messages.is_empty(),
        "a canceled press must not start playback"
    );

    let _ = pad.update(
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        inside(),
    );
    let (messages, _) = pad.update(
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        inside(),
    );
    assert_eq!(messages, [Action::Play]);
}

#[test]
fn interrupted_motion_keeps_face_shadow_and_brightness_continuous() {
    let mut state = State::new(true);
    let start = state.now;
    state.transition(Phase::Hovered, start);
    state.now = start + Duration::from_millis(100);
    let before_press = state.layers();
    state.transition(Phase::Pressed, state.now);
    let after_press = state.layers();
    assert!((after_press.front - before_press.front).abs() < 0.001);
    assert!((after_press.shadow - before_press.shadow).abs() < 0.001);
    assert!((after_press.brightness - before_press.brightness).abs() < 0.001);

    state.now += Duration::from_millis(16);
    let before_release = state.layers();
    state.transition(Phase::Hovered, state.now);
    let after_release = state.layers();
    assert!((after_release.front - before_release.front).abs() < 0.001);
    assert!((after_release.shadow - before_release.shadow).abs() < 0.001);
    assert!((after_release.brightness - before_release.brightness).abs() < 0.001);

    state.now += Duration::from_millis(300);
    let settled = state.layers();
    assert!((settled.front + 6.0).abs() < 0.001);
    assert!((settled.shadow - 4.0).abs() < 0.001);
    assert!(!state.motion.is_animating(state.now));
}
