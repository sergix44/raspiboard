use super::{Message, Soundboard};
use iced::{Size, Task, window};
use raspiboard::APP_ID;
use std::sync::LazyLock;

static APPLICATION_ICON: LazyLock<window::Icon> = LazyLock::new(|| {
    let pixels = image::load_from_memory_with_format(
        include_bytes!("../../img/icon_full.png"),
        image::ImageFormat::Png,
    )
    .expect("The embedded application icon must be a valid PNG")
    .resize(256, 256, image::imageops::FilterType::Lanczos3)
    .into_rgba8();
    let (width, height) = pixels.dimensions();
    window::icon::from_rgba(pixels.into_raw(), width, height)
        .expect("The application icon must contain valid RGBA pixels")
});

pub(super) fn window_settings() -> window::Settings {
    window::Settings {
        icon: Some(APPLICATION_ICON.clone()),
        platform_specific: window::settings::PlatformSpecific {
            application_id: APP_ID.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

impl Soundboard {
    pub fn start() -> (Self, Task<Message>) {
        let mut board = Self::boot();
        let (id, open) = window::open(window::Settings {
            size: Size::new(1100.0, 680.0),
            min_size: Some(Size::new(1000.0, 360.0)),
            ..window_settings()
        });
        board.main_window = Some(id);
        (board, open.discard())
    }

    pub fn title(&self, id: window::Id) -> String {
        if self.options_window == Some(id) {
            "Raspiboard — Sound options".into()
        } else if self.settings_window == Some(id) {
            "Raspiboard — Settings".into()
        } else {
            concat!("Raspiboard v", env!("CARGO_PKG_VERSION")).into()
        }
    }

    pub(super) fn open_settings(&mut self) -> Task<Message> {
        if let Some(id) = self.settings_window {
            return window::minimize(id, false).chain(window::gain_focus(id));
        }
        let (id, open) = window::open(window::Settings {
            size: Size::new(440.0, 420.0),
            min_size: Some(Size::new(360.0, 260.0)),
            position: window::Position::Centered,
            ..window_settings()
        });
        self.settings_window = Some(id);
        open.discard()
    }

    pub(super) fn close_settings(&self) -> Task<Message> {
        self.settings_window
            .map(window::close)
            .unwrap_or_else(Task::none)
    }
}
