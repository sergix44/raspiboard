use super::{
    Message,
    icons::{self, Icon},
    sound_pad,
};
use iced::advanced::{image as image_renderer, renderer, text as text_renderer};
use iced::widget::{button, column, container, image, mouse_area, row, space, text, text_input};
use iced::{Border, Center, Element, Fill, Font, Theme, mouse, window};
use raspiboard::theme::tokens;
use std::sync::LazyLock;

pub(super) const HEIGHT: f32 = 40.0;
const RESIZE_BORDER: f32 = 6.0;
static LOGO: LazyLock<image::Handle> = LazyLock::new(|| {
    image::Handle::from_bytes(include_bytes!("../../img/icon_full.png").as_slice())
});

pub(super) fn view<'a, Renderer>(
    query: &'a str,
    maximized: bool,
) -> Element<'a, Message, Theme, Renderer>
where
    Renderer: text_renderer::Renderer<Font = Font>
        + image_renderer::Renderer<Handle = image::Handle>
        + 'a,
{
    let brand = row![
        image(LOGO.clone()).width(30).height(30),
        text(concat!("Raspiboard v", env!("CARGO_PKG_VERSION"))).size(14),
    ]
    .spacing(10)
    .align_y(Center)
    .width(Fill);
    let input = text_input("Search…", query)
        .id("sound-search")
        .on_input(Message::SearchChanged)
        .size(14)
        .padding([5, 8])
        .icon(text_input::Icon {
            font: icons::FONT,
            code_point: Icon::Search.glyph(),
            size: Some(16.into()),
            spacing: 8.0,
            side: text_input::Side::Left,
        })
        .style(|theme: &Theme, status| {
            let mut style = text_input::default(theme, status);
            style.background = tokens(theme).background.into();
            style.border.width = 0.0;
            style
        });
    let clear: Element<'_, Message, Theme, Renderer> = if query.is_empty() {
        space().width(28).height(28).into()
    } else {
        clear_button().into()
    };
    let search = mouse_area(
        container(row![input, clear].align_y(Center))
            .width(320)
            .padding(4)
            .style(|theme: &Theme| container::Style {
                background: Some(tokens(theme).background.into()),
                border: Border {
                    color: tokens(theme).border,
                    width: 1.0,
                    radius: tokens(theme).radius.into(),
                },
                ..Default::default()
            }),
    )
    .on_press(Message::FocusSearch);
    let controls = container(
        row![
            window_control(
                Icon::Minimize,
                Message::MinimizeMainWindow,
                sound_pad::Tone::Pause
            ),
            window_control(
                if maximized {
                    Icon::Restore
                } else {
                    Icon::Maximize
                },
                Message::ToggleMaximize,
                sound_pad::Tone::Play,
            ),
            window_control(
                Icon::Close,
                Message::CloseMainWindow,
                sound_pad::Tone::Custom(iced::color!(0xef6b73))
            ),
        ]
        .spacing(10),
    )
    .align_right(Fill);
    mouse_area(
        container(row![brand, search, controls].align_y(Center))
            .width(Fill)
            .height(HEIGHT)
            .padding([0, 12])
            .style(|theme: &Theme| container::Style {
                background: Some(tokens(theme).background.into()),
                ..Default::default()
            }),
    )
    .on_press(Message::DragMainWindow)
    .on_double_click(Message::ToggleMaximize)
    .into()
}

fn window_control<'a, Renderer>(
    glyph: Icon,
    action: Message,
    tone: sound_pad::Tone,
) -> Element<'a, Message, Theme, Renderer>
where
    Renderer: text_renderer::Renderer<Font = Font> + 'a,
{
    sound_pad::circular(
        button(
            text(glyph.glyph().to_string())
                .font(icons::FONT)
                .size(15)
                .width(Fill)
                .height(Fill)
                .center(),
        )
        .padding(7)
        .on_press(action),
        32.0,
        tone,
    )
}

fn clear_button<'a, Renderer>() -> button::Button<'a, Message, Theme, Renderer>
where
    Renderer: text_renderer::Renderer<Font = Font> + 'a,
{
    button(
        text(Icon::Close.glyph().to_string())
            .font(icons::FONT)
            .size(16)
            .width(Fill)
            .height(Fill)
            .center(),
    )
    .width(28)
    .height(28)
    .padding(6)
    .on_press(Message::SearchChanged(String::new()))
    .style(|theme: &Theme, status| {
        let colors = tokens(theme);
        let background = match status {
            button::Status::Hovered => Some(colors.raised.into()),
            button::Status::Pressed => Some(colors.illuminated.into()),
            _ => None,
        };
        button::Style {
            background,
            text_color: colors.text,
            border: Border {
                radius: colors.radius.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
}

pub(super) fn frame<'a, Renderer: renderer::Renderer + 'a>(
    content: Element<'a, Message, Theme, Renderer>,
    maximized: bool,
) -> Element<'a, Message, Theme, Renderer> {
    use window::Direction::*;
    let top = row![
        resize_handle(NorthWest, maximized)
            .width(12)
            .height(RESIZE_BORDER),
        resize_handle(North, maximized)
            .width(Fill)
            .height(RESIZE_BORDER),
        resize_handle(NorthEast, maximized)
            .width(12)
            .height(RESIZE_BORDER),
    ];
    let bottom = row![
        resize_handle(SouthWest, maximized)
            .width(12)
            .height(RESIZE_BORDER),
        resize_handle(South, maximized)
            .width(Fill)
            .height(RESIZE_BORDER),
        resize_handle(SouthEast, maximized)
            .width(12)
            .height(RESIZE_BORDER),
    ];
    let frame = column![
        top,
        row![
            resize_handle(West, maximized)
                .width(RESIZE_BORDER)
                .height(Fill),
            content,
            resize_handle(East, maximized)
                .width(RESIZE_BORDER)
                .height(Fill),
        ]
        .height(Fill),
        bottom,
    ]
    .width(Fill)
    .height(Fill);
    container(frame)
        .width(Fill)
        .height(Fill)
        .style(move |theme: &Theme| container::Style {
            background: Some(tokens(theme).background.into()),
            border: Border {
                color: tokens(theme).border,
                width: if maximized { 0.0 } else { 1.0 },
                radius: if maximized { 0.0 } else { tokens(theme).radius }.into(),
            },
            ..Default::default()
        })
        .into()
}

fn resize_handle<'a, Renderer: renderer::Renderer + 'a>(
    direction: window::Direction,
    maximized: bool,
) -> container::Container<'a, Message, Theme, Renderer> {
    use window::Direction::*;
    let area = mouse_area(space().width(Fill).height(Fill));
    let area = if maximized {
        area
    } else {
        area.on_press(Message::ResizeMainWindow(direction))
            .interaction(match direction {
                North | South => mouse::Interaction::ResizingVertically,
                East | West => mouse::Interaction::ResizingHorizontally,
                NorthWest | SouthEast => mouse::Interaction::ResizingDiagonallyDown,
                NorthEast | SouthWest => mouse::Interaction::ResizingDiagonallyUp,
            })
    };
    container(area)
}

#[cfg(test)]
mod tests;
