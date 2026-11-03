use super::sound_pad::shade;
use iced::advanced::{Layout, Widget, layout, mouse, renderer, widget::Tree};
use iced::widget::slider;
use iced::{Border, Element, Length, Rectangle, Size, Theme, gradient};
use raspiboard::theme::tokens;

pub(super) fn view<'a, Message: Clone + 'a, Renderer: renderer::Renderer + 'a>(
    value: u8,
    on_change: impl Fn(u8) -> Message + 'a,
    enabled: bool,
) -> Element<'a, Message, Theme, Renderer> {
    let content = slider(0..=100, value, on_change)
        .width(160)
        .height(22)
        .style(move |theme, status| slider_style(theme, status, enabled))
        .into();
    if enabled {
        content
    } else {
        Element::new(Disabled { content })
    }
}

// iced's slider has no disabled mode; keep its rendering while ignoring input.
struct Disabled<'a, Message, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

impl<Message, Renderer: renderer::Renderer> Widget<Message, Theme, Renderer>
    for Disabled<'_, Message, Renderer>
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[self.content.as_widget()]);
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

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            mouse::Cursor::Unavailable,
            viewport,
        );
    }
}

pub(super) fn style(theme: &Theme, status: slider::Status) -> slider::Style {
    slider_style(theme, status, true)
}

fn slider_style(theme: &Theme, status: slider::Status, enabled: bool) -> slider::Style {
    let tokens = tokens(theme);
    let status = if enabled {
        status
    } else {
        slider::Status::Active
    };
    let accent = if enabled {
        tokens.accent
    } else {
        tokens.border
    };
    let face = shade(
        tokens.raised,
        match status {
            slider::Status::Active => 1.0,
            slider::Status::Hovered => 1.1,
            slider::Status::Dragged => 0.95,
        },
    );
    let edge_start = if status == slider::Status::Dragged {
        0.9
    } else {
        0.75
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                gradient::Linear::new(std::f32::consts::PI)
                    .add_stop(0.0, shade(accent, 0.55))
                    .add_stop(1.0, accent)
                    .into(),
                gradient::Linear::new(std::f32::consts::PI)
                    .add_stop(0.0, tokens.background)
                    .add_stop(1.0, tokens.surface)
                    .into(),
            ),
            width: 8.0,
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: tokens.border,
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Rectangle {
                width: 18,
                border_radius: tokens.radius.into(),
            },
            background: gradient::Linear::new(std::f32::consts::PI)
                .add_stop(0.0, shade(face, if tokens.metallic { 1.4 } else { 1.1 }))
                .add_stop(edge_start, face)
                .add_stop(
                    edge_start + 0.01,
                    shade(face, if tokens.metallic { 0.65 } else { 0.85 }),
                )
                .add_stop(1.0, shade(face, if tokens.metallic { 0.45 } else { 0.75 }))
                .into(),
            border_width: 1.0,
            border_color: tokens.border,
        },
    }
}

#[cfg(test)]
mod tests;
