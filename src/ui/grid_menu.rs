use super::{
    Message,
    icons::{Icon, icon},
};
use iced::widget::{button, column, container, float, mouse_area, opaque, row, space, stack, text};
use iced::{Border, Center, Color, Element, Fill, Point, Shadow, Theme, Vector};
use raspiboard::theme::tokens;

pub(super) fn area(base: Element<'_, Message>) -> Element<'_, Message> {
    mouse_area(base)
        .on_move(Message::GridPointerMoved)
        .on_right_press(Message::OpenGridMenu)
        .into()
}

pub(super) fn view<'a>(
    base: Element<'a, Message>,
    position: Option<Point>,
    button_id: Option<&str>,
) -> Element<'a, Message> {
    let Some(position) = position else {
        return base;
    };
    let dismiss = opaque(
        mouse_area(space().width(Fill).height(Fill))
            .on_press(Message::DismissMenu)
            .on_right_press(Message::DismissMenu),
    );
    let items = if let Some(id) = button_id {
        vec![
            (
                Icon::Settings,
                "Options",
                Message::OpenButtonOptions(id.into()),
            ),
            (
                Icon::Replace,
                "Replace sound",
                Message::OpenAudio(id.into()),
            ),
            (Icon::Delete, "Delete", Message::DeleteButton(id.into())),
        ]
    } else {
        vec![(Icon::Add, "Add Sound", Message::AddSound)]
    };
    let items = column(items.into_iter().map(|(glyph, label, action)| {
        button(
            row![
                icon(glyph).size(16).width(16),
                text(label)
                    .size(12)
                    .width(Fill)
                    .height(Fill)
                    .align_y(Center),
            ]
            .spacing(8)
            .align_y(Center),
        )
        .on_press(action)
        .width(Fill)
        .height(28)
        .padding([4, 8])
        .style(item_style)
        .into()
    }));
    let menu = float(container(items).width(176).padding(2).style(panel_style)).translate(
        move |bounds, viewport| {
            let x = (bounds.x + position.x).clamp(
                viewport.x,
                (viewport.x + viewport.width - bounds.width).max(viewport.x),
            );
            let y = (bounds.y + position.y).clamp(
                viewport.y,
                (viewport.y + viewport.height - bounds.height).max(viewport.y),
            );
            Vector::new(x - bounds.x, y - bounds.y)
        },
    );
    stack![base, dismiss, menu].into()
}

fn item_style(theme: &Theme, status: button::Status) -> button::Style {
    let tokens = tokens(theme);
    let highlighted = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: highlighted.then_some(tokens.illuminated.into()),
        text_color: tokens.text,
        border: Border {
            radius: tokens.radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn panel_style(theme: &Theme) -> container::Style {
    let tokens = tokens(theme);
    container::Style {
        background: Some(tokens.surface.into()),
        text_color: Some(tokens.text),
        border: Border {
            radius: tokens.radius.into(),
            width: 1.0,
            color: tokens.border,
        },
        shadow: Shadow {
            color: Color::BLACK.scale_alpha(0.18),
            offset: Vector::new(0.0, 3.0),
            blur_radius: 10.0,
        },
        ..container::Style::default()
    }
}
