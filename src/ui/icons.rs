use iced::widget::{Text, text};
use iced::{Fill, Font};

pub(crate) const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/lucide-0.468.0.ttf");
const FONT: Font = Font::with_name("lucide");

pub(super) enum Icon {
    Play,
    Pause,
    Stop,
    Link,
    Unlink,
    Settings,
    Replace,
    Delete,
    Add,
    ArrowRight,
    Cable,
    Record,
}

pub(super) fn icon(icon: Icon) -> Text<'static> {
    let glyph = match icon {
        Icon::Play => '\u{e13f}',
        Icon::Pause => '\u{e131}',
        Icon::Stop => '\u{e16a}',
        Icon::Link => '\u{e107}',
        Icon::Unlink => '\u{e19b}',
        Icon::Settings => '\u{e157}',
        Icon::Replace => '\u{e148}',
        Icon::Delete => '\u{e18d}',
        Icon::Add => '\u{e140}',
        Icon::ArrowRight => '\u{e04d}',
        Icon::Cable => '\u{e4e7}',
        Icon::Record => '\u{e07a}',
    };
    text(glyph.to_string())
        .font(FONT)
        .size(20)
        .line_height(1.0)
        .width(Fill)
        .height(Fill)
        .center()
}
