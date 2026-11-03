use super::{Message, Soundboard};
use iced::futures::{SinkExt, Stream};
use iced::{Task, window};
use ksni::TrayMethods;

#[derive(Debug, Clone)]
pub enum Event {
    Available(bool),
    Show,
    Quit,
}

struct Tray {
    events: async_channel::Sender<Event>,
    icon: ksni::Icon,
}

impl Tray {
    fn send(&self, event: Event) {
        let _ = self.events.try_send(event);
    }
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        raspiboard::APP_ID.into()
    }

    fn title(&self) -> String {
        "Raspiboard".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(Event::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: "Show Raspiboard".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Event::Show)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            ksni::menu::StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| tray.send(Event::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_online(&self) {
        self.send(Event::Available(true));
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        self.send(Event::Available(false));
        true
    }
}

pub fn events() -> impl Stream<Item = Event> {
    iced::stream::channel(8, async |mut output| {
        let (events, receiver) = async_channel::unbounded();
        let pixels = image::load_from_memory_with_format(
            include_bytes!("../../img/icon_full.png"),
            image::ImageFormat::Png,
        )
        .expect("The embedded application icon must be a valid PNG")
        .resize(64, 64, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
        let icon = ksni::Icon {
            width: pixels.width() as i32,
            height: pixels.height() as i32,
            data: pixels
                .pixels()
                .flat_map(|p| [p[3], p[0], p[1], p[2]])
                .collect(),
        };
        let handle = match (Tray { events, icon }).spawn().await {
            Ok(handle) => handle,
            Err(error) => {
                log::warn!(target: "raspiboard::tray", "Tray unavailable: {error}");
                let _ = output.send(Event::Available(false)).await;
                return;
            }
        };
        if output.send(Event::Available(true)).await.is_err() {
            handle.shutdown().await;
            return;
        }
        while let Ok(event) = receiver.recv().await {
            if output.send(event).await.is_err() {
                break;
            }
        }
        handle.shutdown().await;
    })
}

impl Soundboard {
    pub(super) fn close_requested(&mut self, id: window::Id) -> Task<Message> {
        if self.main_window != Some(id) {
            return Task::none();
        }
        if !self.minimize_to_tray || !self.tray_available {
            return self.quit();
        }
        self.main_hidden = true;
        self.main_window = None;
        self.reorder = None;
        self.grid_menu = None;
        self.button_menu = None;
        Task::batch([
            window::close(id),
            self.close_settings(),
            self.close_button_options(),
        ])
    }

    pub(super) fn tray_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Available(available) => {
                self.tray_available = available;
                if !available && self.main_hidden {
                    return self.open_main();
                }
                Task::none()
            }
            Event::Show => self.open_main(),
            Event::Quit => self.quit(),
        }
    }

    pub(super) fn quit(&mut self) -> Task<Message> {
        if self.quitting {
            return Task::none();
        }
        if let Some(controller) = self.controller.clone() {
            self.quitting = true;
            return Task::perform(controller.finish_recording(), Message::RecordingFinished);
        }
        self.finish_quit()
    }

    pub(super) fn finish_quit(&mut self) -> Task<Message> {
        self.reorder = None;
        self.main_hidden = false;
        self.main_window = None;
        self.settings_window = None;
        self.options_window = None;
        self.options_button = None;
        self.options.cancel_analysis();
        iced::exit()
    }
}
