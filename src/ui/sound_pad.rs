use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::animation::{Animation, Easing, Interpolable};
use iced::time::{Duration, Instant};
use iced::widget::button;
use iced::{
    Border, Color, Element, Event, Length, Rectangle, Shadow, Size, Theme, Vector, gradient, touch,
    window,
};
use raspiboard::theme::tokens;

const CIRCULAR_LIFT: f32 = 8.0;

#[derive(Clone, Copy)]
pub(super) enum Tone {
    Accent,
    Custom(Color),
    Play,
    Pause,
    Stop,
}

pub(super) fn animated<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    content: button::Button<'a, Message, Theme, Renderer>,
    enabled: bool,
    playing: bool,
) -> Element<'a, Message, Theme, Renderer> {
    colored(content, enabled, playing, None, false)
}

pub(super) fn colored<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    content: button::Button<'a, Message, Theme, Renderer>,
    enabled: bool,
    playing: bool,
    color: Option<Color>,
    overlap: bool,
) -> Element<'a, Message, Theme, Renderer> {
    raised(
        content,
        enabled,
        playing,
        color.map_or(Tone::Accent, Tone::Custom),
        overlap,
    )
}

pub(super) use raspiboard::storage::parse_button_color as parse_color;

pub(super) fn color_hex(color: Color) -> String {
    let [red, green, blue, _] = color.into_rgba8();
    format!("#{red:02X}{green:02X}{blue:02X}")
}

pub(super) fn raised<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    content: button::Button<'a, Message, Theme, Renderer>,
    enabled: bool,
    playing: bool,
    tone: Tone,
    overlap: bool,
) -> Element<'a, Message, Theme, Renderer> {
    pad(content, enabled, playing, tone, overlap, false, false)
}

pub(super) fn circular<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    content: button::Button<'a, Message, Theme, Renderer>,
    diameter: f32,
    tone: Tone,
) -> Element<'a, Message, Theme, Renderer> {
    // Keep the raised face's full animation inside the button's input bounds.
    pad(
        content.width(diameter).height(diameter + CIRCULAR_LIFT),
        true,
        false,
        tone,
        false,
        false,
        true,
    )
}

pub(super) fn blinking<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    content: button::Button<'a, Message, Theme, Renderer>,
    enabled: bool,
    active: bool,
    color: Color,
) -> Element<'a, Message, Theme, Renderer> {
    pad(
        content,
        enabled,
        active,
        Tone::Custom(color),
        false,
        true,
        false,
    )
}

fn pad<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    content: button::Button<'a, Message, Theme, Renderer>,
    enabled: bool,
    playing: bool,
    tone: Tone,
    overlap: bool,
    blink: bool,
    circular: bool,
) -> Element<'a, Message, Theme, Renderer> {
    Element::new(AnimatedPad {
        content: content
            .style(move |theme, status| style(theme, status, playing, tone))
            .into(),
        enabled,
        playing,
        tone,
        overlap,
        blink,
        circular,
    })
}

fn style(theme: &Theme, status: button::Status, playing: bool, tone: Tone) -> button::Style {
    let enabled = status != button::Status::Disabled;
    let pair = colors(theme, enabled, playing, tone);
    button::Style {
        text_color: if enabled {
            pair.text
        } else {
            pair.text.scale_alpha(0.65)
        },
        snap: false,
        ..button::Style::default()
    }
}

fn colors(theme: &Theme, enabled: bool, playing: bool, tone: Tone) -> iced::theme::palette::Pair {
    let palette = theme.extended_palette();
    let tokens = tokens(theme);
    let colored = match tone {
        Tone::Custom(color) => Some(iced::theme::palette::Pair::new(color, tokens.text)),
        Tone::Play => Some(palette.success.base),
        Tone::Pause => Some(iced::theme::palette::Pair::new(
            iced::color!(0xfacc15),
            Color::BLACK,
        )),
        Tone::Stop => Some(iced::theme::palette::Pair::new(
            iced::color!(0x7dd3fc),
            Color::BLACK,
        )),
        Tone::Accent => None,
    };
    if let Some(mut pair) = colored {
        if !enabled {
            pair.color = pair.color.scale_alpha(0.4);
        }
        pair
    } else {
        iced::theme::palette::Pair::new(
            if !enabled {
                tokens.surface
            } else if playing {
                tokens.illuminated
            } else {
                tokens.raised
            },
            tokens.text,
        )
    }
}

fn backlight(theme: &Theme, enabled: bool, playing: bool, tone: Tone) -> Option<Color> {
    (enabled && playing).then(|| match tone {
        Tone::Custom(color) => color,
        _ => tokens(theme).accent,
    })
}

pub(super) fn shade(color: Color, factor: f32) -> Color {
    Color {
        r: (color.r * factor).clamp(0.0, 1.0),
        g: (color.g * factor).clamp(0.0, 1.0),
        b: (color.b * factor).clamp(0.0, 1.0),
        ..color
    }
}

pub(super) fn face_background(theme: &Theme, face: Color) -> iced::Background {
    if tokens(theme).metallic {
        gradient::Linear::new(std::f32::consts::FRAC_PI_2)
            .add_stop(0.0, shade(face, 1.32))
            .add_stop(0.12, shade(face, 1.12))
            .add_stop(0.48, face)
            .add_stop(0.52, shade(face, 0.94))
            .add_stop(1.0, shade(face, 0.78))
            .into()
    } else {
        gradient::Linear::new(std::f32::consts::FRAC_PI_2)
            .add_stop(0.0, shade(face, 1.10))
            .add_stop(1.0, shade(face, 0.94))
            .into()
    }
}

fn blink_intensity(elapsed: Duration) -> f32 {
    let ms = (elapsed.as_micros() % 2_000_000) as f32 / 1000.0;
    if ms < 40.0 {
        let warmup = ms / 40.0;
        warmup * (2.0 - warmup)
    } else if ms < 1000.0 {
        1.0
    } else if ms < 1060.0 {
        let afterglow = 1.0 - (ms - 1000.0) / 60.0;
        afterglow * afterglow
    } else {
        0.0
    }
}

fn quad(bounds: Rectangle, radius: f32) -> renderer::Quad {
    renderer::Quad {
        bounds,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        snap: false,
        ..renderer::Quad::default()
    }
}

struct AnimatedPad<'a, Message, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    enabled: bool,
    playing: bool,
    tone: Tone,
    overlap: bool,
    blink: bool,
    circular: bool,
}

#[derive(Clone, Copy)]
struct Layers {
    front: f32,
    shadow: f32,
    brightness: f32,
}

impl Interpolable for Layers {
    fn interpolated(&self, other: Self, ratio: f32) -> Self {
        Self {
            front: self.front.interpolated(other.front, ratio),
            shadow: self.shadow.interpolated(other.shadow, ratio),
            brightness: self.brightness.interpolated(other.brightness, ratio),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Rest,
    Hovered,
    Pressed,
    Disabled,
}

impl Phase {
    fn layers(self) -> Layers {
        let (front, shadow, brightness) = match self {
            Self::Rest => (-4.0, 2.0, 1.0),
            Self::Hovered => (-6.0, 4.0, 1.1),
            Self::Pressed => (-2.0, 1.0, 1.1),
            Self::Disabled => (0.0, 0.0, 1.0),
        };
        Layers {
            front,
            shadow,
            brightness,
        }
    }

    fn timing(self) -> (Duration, Easing) {
        match self {
            Self::Pressed => (Duration::from_millis(34), Easing::EaseInOut),
            Self::Hovered => (Duration::from_millis(250), Easing::EaseOutBack),
            Self::Rest => (Duration::from_millis(600), Easing::EaseOutCubic),
            Self::Disabled => (Duration::from_millis(34), Easing::EaseOutCubic),
        }
    }
}

struct State {
    motion: Animation<bool>,
    phase: Phase,
    from: Layers,
    to: Layers,
    now: Instant,
    pressed: bool,
    enabled: bool,
    blink_started: Option<Instant>,
}

impl State {
    fn new(enabled: bool) -> Self {
        let phase = if enabled {
            Phase::Rest
        } else {
            Phase::Disabled
        };
        Self {
            motion: Animation::new(true),
            phase,
            from: phase.layers(),
            to: phase.layers(),
            now: Instant::now(),
            pressed: false,
            enabled,
            blink_started: None,
        }
    }

    fn offset(&self) -> f32 {
        self.layers().front
    }

    fn layers(&self) -> Layers {
        self.motion.interpolate(self.from, self.to, self.now)
    }

    fn transition(&mut self, phase: Phase, now: Instant) {
        self.now = now;
        if self.phase != phase {
            // Sample before changing timing so interrupted motion stays continuous.
            self.from = self.layers();
            self.to = phase.layers();
            self.phase = phase;
            let (duration, easing) = phase.timing();
            self.motion = Animation::new(false)
                .duration(duration)
                .easing(easing)
                .go(true, now);
        }
    }
}

impl<Message, Renderer: renderer::Renderer> Widget<Message, Theme, Renderer>
    for AnimatedPad<'_, Message, Renderer>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.enabled))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        let cancel_press = state.enabled && !self.enabled;
        state.enabled = self.enabled;
        if cancel_press {
            state.pressed = false;
            // iced retains a Button's pressed state when it becomes disabled.
            tree.children = self.children();
        } else {
            tree.diff_children(std::slice::from_ref(&self.content));
        }
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tree.state.downcast_mut::<State>();
        let hovered = self.enabled && cursor.is_over(layout.bounds());
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if hovered =>
            {
                state.pressed = true;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                state.pressed = false;
            }
            _ => {}
        }
        let now = match event {
            Event::Window(window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };
        let phase = if !self.enabled {
            Phase::Disabled
        } else if !hovered {
            Phase::Rest
        } else if state.pressed {
            Phase::Pressed
        } else {
            Phase::Hovered
        };
        state.transition(phase, now);
        if self.blink && self.enabled && self.playing {
            state.blink_started.get_or_insert(now);
            shell.request_redraw_at(now + Duration::from_millis(33));
        } else {
            state.blink_started = None;
        }
        if state.motion.is_animating(state.now) {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let tokens = tokens(theme);
        let layers = state.layers();
        let bounds = if self.circular {
            Rectangle {
                y: layout.bounds().y + CIRCULAR_LIFT,
                height: layout.bounds().height - CIRCULAR_LIFT,
                ..layout.bounds()
            }
        } else {
            layout.bounds()
        };
        let radius = if self.circular {
            bounds.width.min(bounds.height) / 2.0
        } else {
            tokens.radius
        };
        let blink = state.blink_started.map_or(1.0, |start| {
            blink_intensity(state.now.saturating_duration_since(start))
        });
        let face = shade(
            colors(theme, self.enabled, self.playing, self.tone).color,
            layers.brightness * (0.45 + 0.55 * blink),
        );
        let light = backlight(theme, self.enabled, self.playing, self.tone)
            .filter(|_| blink > 0.0)
            .map(|color| color.scale_alpha(blink));
        if let Some(light) = light {
            let backlight = Rectangle {
                x: bounds.x - 1.0,
                y: bounds.y - 1.0,
                width: bounds.width + 2.0,
                height: bounds.height + 2.0,
            };
            renderer.fill_quad(
                renderer::Quad {
                    border: Border {
                        color: light.scale_alpha(0.7),
                        width: 1.0,
                        radius: (radius + 1.0).into(),
                    },
                    shadow: Shadow {
                        color: light.scale_alpha(0.7),
                        blur_radius: 14.0,
                        ..Shadow::default()
                    },
                    ..quad(backlight, radius + 1.0)
                },
                light.scale_alpha(0.18),
            );
        }
        if self.enabled {
            renderer.fill_quad(
                renderer::Quad {
                    shadow: Shadow {
                        color: Color::BLACK.scale_alpha(0.12),
                        blur_radius: 2.0,
                        ..Shadow::default()
                    },
                    ..quad(bounds + Vector::new(0.0, layers.shadow), radius)
                },
                Color::BLACK.scale_alpha(0.25),
            );
        }
        let edge = if self.enabled {
            gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                .add_stop(0.0, shade(face, 0.34))
                .add_stop(0.08, shade(face, 0.68))
                .add_stop(0.92, shade(face, 0.68))
                .add_stop(1.0, shade(face, 0.34))
                .into()
        } else {
            iced::Background::Color(face)
        };
        renderer.fill_quad(quad(bounds, radius), edge);
        let surface: iced::Background = if matches!(self.tone, Tone::Custom(_)) {
            face.into()
        } else {
            face_background(theme, face)
        };
        renderer.fill_quad(
            renderer::Quad {
                border: Border {
                    color: light.unwrap_or(tokens.border),
                    width: 1.0,
                    radius: radius.into(),
                },
                ..quad(bounds + Vector::new(0.0, state.offset()), radius)
            },
            surface,
        );
        // Keep input bounds fixed while the pad's surface moves.
        let content_offset = state.offset()
            + if self.circular {
                CIRCULAR_LIFT / 2.0
            } else {
                0.0
            };
        renderer.with_translation(Vector::new(0.0, content_offset), |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            );
            if self.overlap {
                let dot = Rectangle {
                    x: bounds.x + 8.0,
                    y: bounds.y + bounds.height - 12.0,
                    width: 4.0,
                    height: 4.0,
                };
                renderer.fill_quad(
                    quad(dot, 2.0),
                    colors(theme, self.enabled, self.playing, self.tone).text,
                );
            }
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests;
