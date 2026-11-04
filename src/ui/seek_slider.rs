use super::sound_pad::{face_background, shade};
use iced::advanced::{
    Layout, Widget, layout, mouse, renderer,
    widget::{Tree, tree},
};
use iced::animation::{Animation, Easing};
use iced::time::{Duration, Instant};
use iced::widget::slider;
use iced::{Border, Element, Event, Fill, Length, Rectangle, Size, Theme, gradient, touch, window};
use raspiboard::theme::tokens;
use std::ops::RangeInclusive;

const INPUT_HEIGHT: f32 = 16.0;

pub(super) fn stack_panel<'a, Message: 'a, Renderer: renderer::Renderer + 'a>(
    progress: Element<'a, Message, Theme, Renderer>,
    panel: Element<'a, Message, Theme, Renderer>,
) -> Element<'a, Message, Theme, Renderer> {
    // Stack puts the panel in a later render layer than the clipped progress.
    iced::widget::stack![iced::widget::container(panel).padding(iced::Padding {
        top: INPUT_HEIGHT,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    })]
    .push_under(progress)
    .into()
}

pub(super) fn view<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    range: RangeInclusive<u64>,
    position: u64,
    playback_id: Option<u64>,
    on_change: impl Fn(u64) -> Message + 'a,
    on_release: Message,
    on_cancel: Message,
) -> Element<'a, Message, Theme, Renderer> {
    let progress = (position.saturating_sub(*range.start()) as f64
        / range.end().saturating_sub(*range.start()).max(1) as f64)
        .clamp(0.0, 1.0) as f32;
    // Reuse native slider gestures; paint the progress without a slider handle.
    let content = slider(
        *range.start() as f64..=*range.end() as f64,
        position as f64,
        move |position| on_change(position.round() as u64),
    )
    .step(1.0)
    .on_release(on_release)
    .width(Fill)
    .height(INPUT_HEIGHT)
    .into();
    Element::new(ProgressBar {
        content,
        on_cancel,
        playback_id,
        progress,
    })
}

struct ProgressBar<'a, Message, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_cancel: Message,
    playback_id: Option<u64>,
    progress: f32,
}

struct State {
    source: Option<u64>,
    reveal: Animation<bool>,
    expand: Animation<bool>,
    now: Instant,
    hovered: bool,
    dragging: bool,
    progress: f32,
}

impl State {
    fn new(source: Option<u64>, progress: f32) -> Self {
        let now = Instant::now();
        Self {
            source,
            reveal: Animation::new(false)
                .duration(Duration::from_millis(220))
                .easing(Easing::EaseOutCubic)
                .go(source.is_some(), now),
            expand: Animation::new(false)
                .duration(Duration::from_millis(140))
                .easing(Easing::EaseOutCubic),
            now,
            hovered: false,
            dragging: false,
            progress,
        }
    }
}

impl<Message: Clone, Renderer: renderer::Renderer> Widget<Message, Theme, Renderer>
    for ProgressBar<'_, Message, Renderer>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.playback_id, self.progress))
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        state.now = Instant::now();
        let changed = state.source != self.playback_id;
        if changed {
            state.source = self.playback_id;
            state.dragging = false;
        }
        if self.playback_id.is_some() {
            state.progress = self.progress;
        }
        state.reveal.go_mut(self.playback_id.is_some(), state.now);
        state.expand.go_mut(
            self.playback_id.is_some() && (state.hovered || state.dragging),
            state.now,
        );
        if changed {
            tree.children = self.children();
        } else {
            tree.diff_children(&[self.content.as_widget()]);
        }
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
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        state.now = match event {
            Event::Window(window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };
        let was_hovered = state.hovered;
        state.hovered = self.playback_id.is_some() && cursor.is_over(layout.bounds());
        if matches!(event, Event::Window(window::Event::Unfocused)) {
            state.dragging = false;
            state.hovered = false;
            tree.children[0] = Tree::new(self.content.as_widget());
            shell.publish(self.on_cancel.clone());
        } else if self.playback_id.is_some() {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
                    if state.hovered =>
                {
                    state.dragging = true
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                | Event::Touch(
                    touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
                ) => state.dragging = false,
                _ => {}
            }
            // Native keyboard/wheel changes have no release event to commit a seek.
            if !matches!(
                event,
                Event::Keyboard(_) | Event::Mouse(mouse::Event::WheelScrolled { .. })
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
            }
        }
        state
            .expand
            .go_mut(state.hovered || state.dragging, state.now);
        if was_hovered != state.hovered
            || state.reveal.is_animating(state.now)
            || state.expand.is_animating(state.now)
        {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let reveal = state.reveal.interpolate(0.0_f32, 1.0, state.now);
        if reveal <= 0.0 {
            return;
        }
        let height = state.expand.interpolate(3.0_f32, 10.0, state.now);
        let area = layout.bounds();
        let colors = tokens(theme);
        let overlap = colors.radius + 1.0;
        let bounds = Rectangle {
            y: area.y + area.height - height + (1.0 - reveal) * 12.0,
            height: height + overlap,
            ..area
        };
        let radius = iced::border::top(colors.radius);
        let quad = |bounds| renderer::Quad {
            bounds,
            border: Border {
                radius,
                ..Border::default()
            },
            snap: false,
            ..renderer::Quad::default()
        };
        // The following panel paints over the overlap, retaining its rounded corners.
        renderer.with_layer(
            Rectangle {
                height: area.height + overlap,
                ..area
            },
            |renderer| {
                renderer.fill_quad(
                    renderer::Quad {
                        border: Border {
                            color: colors.border,
                            width: 0.5,
                            radius,
                        },
                        ..quad(bounds)
                    },
                    colors.surface,
                );
                let fill = Rectangle {
                    width: bounds.width * state.progress,
                    ..bounds
                };
                if fill.width <= 0.0 {
                    return;
                }
                let face = shade(
                    theme.extended_palette().warning.base.color,
                    if state.dragging {
                        0.95
                    } else if state.hovered {
                        1.1
                    } else {
                        1.0
                    },
                );
                renderer.fill_quad(
                    quad(fill),
                    gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                        .add_stop(0.0, shade(face, 0.34))
                        .add_stop(0.08, shade(face, 0.68))
                        .add_stop(0.92, shade(face, 0.68))
                        .add_stop(1.0, shade(face, 0.34)),
                );
                renderer.fill_quad(
                    renderer::Quad {
                        border: Border {
                            color: shade(face, 0.75),
                            width: 0.5,
                            radius,
                        },
                        ..quad(Rectangle {
                            height: (bounds.height - 1.0).max(0.0),
                            ..fill
                        })
                    },
                    face_background(theme, face),
                );
            },
        );
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.playback_id.is_some() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests;
