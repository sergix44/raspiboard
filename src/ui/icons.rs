use iced::widget::{Text, text};
use iced::{Fill, Font};

pub(crate) const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/lucide-0.468.0.ttf");
pub(super) const FONT: Font = Font::with_name("lucide");

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
    Search,
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Icon {
    pub(super) fn glyph(self) -> char {
        match self {
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
            Icon::Search => '\u{e154}',
            Icon::Minimize => '\u{e11f}',
            Icon::Maximize => '\u{e115}',
            Icon::Restore => '\u{e0a2}',
            Icon::Close => '\u{e1b1}',
        }
    }
}

pub(super) fn icon(icon: Icon) -> Text<'static> {
    text(icon.glyph().to_string())
        .font(FONT)
        .size(20)
        .line_height(1.0)
        .width(Fill)
        .height(Fill)
        .center()
}
