use super::options_state::{format_time, parse_time};
use iced::{Element, Fill, Font, Theme, advanced::text};

pub(super) fn view<'a, Message: Clone + 'a, Renderer: text::Renderer<Font = Font> + 'a>(
    draft: &str,
    default_ms: u64,
    maximum_ms: Option<u64>,
    on_change: impl Fn(String) -> Message + Clone + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    let value = parse_time(draft).ok().flatten().unwrap_or(default_ms) as f64 / 1000.0;
    let maximum = maximum_ms.unwrap_or(u64::MAX) as f64 / 1000.0;
    let cleared = on_change(String::new());
    iced_aw::NumberInput::new(&value, 0.0..=maximum, move |seconds| {
        let ms = (seconds * 1000.0).round() as u64;
        on_change(format_time((ms != default_ms).then_some(ms)))
    })
    .on_clear(cleared)
    .step(0.01)
    .width(Fill)
    .font(super::fonts::FONT)
    .padding(10)
    .style(|theme: &Theme, status| iced_aw::number_input::Style {
        button_background: None,
        icon_color: match status {
            iced_aw::style::Status::Disabled => theme.palette().text.scale_alpha(0.35),
            iced_aw::style::Status::Pressed => theme.palette().primary,
            _ => theme.palette().text,
        },
    })
    .into()
}

#[cfg(test)]
mod tests;
