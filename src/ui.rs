use iced::widget::{button, column, container, grid, pick_list, row, scrollable, space, text};
use iced::{
    Alignment, Center, Element, Fill, Point, Subscription, Task, Theme, event, keyboard, window,
};
use raspiboard::audio::{self, Command, Controller, Event, PlaybackStatus, Snapshot, Target};
use raspiboard::storage::{
    Button, Change, MAX_SOUND_BUTTON_SCALE, MIN_SOUND_BUTTON_SCALE, Report, Writer,
};
use raspiboard::theme::ThemeChoice;
use std::path::{Path, PathBuf};

use icons::{Icon, icon};

mod button_options;
mod crop_input;
mod fitted_label;
pub(crate) mod fonts;
mod gain_slider;
mod grid_menu;
mod hotkeys;
pub(crate) mod icons;
mod options_state;
mod passthrough;
mod persistence;
mod recording;
mod reorder;
mod settings;
mod sound_pad;
#[cfg(test)]
mod test_renderer;
mod tray;
mod volume_slider;
mod waveform;
mod windows;

const CONTENT_PADDING: f32 = 12.0;

pub struct Soundboard {
    theme: ThemeChoice,
    main_window: Option<window::Id>,
    main_hidden: bool,
    tray_available: bool,
    minimize_to_tray: bool,
    recordings: raspiboard::recording::RecordingSettings,
    default_recording_directory: Option<PathBuf>,
    choosing_recording_directory: bool,
    quitting: bool,
    settings_window: Option<window::Id>,
    options_window: Option<window::Id>,
    options_button: Option<String>,
    options: options_state::OptionsState,
    hotkeys: hotkeys::State,
    remote_volume: u8,
    local_volume: u8,
    volumes_linked: bool,
    sound_button_scale: u16,
    controller: Option<Controller>,
    audio: Snapshot,
    buttons: Vec<Button>,
    default_button_count: usize,
    default_category_id: String,
    button_categories: std::collections::HashMap<String, String>,
    reorder: Option<reorder::Drag>,
    active_button: Option<String>,
    choosing_file: Option<String>,
    grid_pointer: Point,
    grid_menu: Option<Point>,
    button_menu: Option<String>,
    error: Option<String>,
    persistence: Option<Writer>,
    storage_error: Option<String>,
    last_saved_revision: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenSettings,
    CloseSettings,
    WindowClosed(window::Id),
    WindowCloseRequested(window::Id),
    MinimizeToTrayChanged(bool),
    Tray(tray::Event),
    EscapePressed(window::Id),
    ThemeChanged(ThemeChoice),
    SoundButtonScaleChanged(u16),
    RemoteVolumeChanged(u8),
    LocalVolumeChanged(u8),
    ToggleVolumeLink,
    Audio(Event),
    Hotkeys(raspiboard::hotkeys::Event),
    SelectTarget(Option<Target>),
    SelectSource(Option<Target>),
    TogglePassthrough,
    ToggleRecording,
    RecordingFormatChanged(raspiboard::recording::RecordingFormat),
    ChooseRecordingDirectory,
    RecordingDirectoryChosen(Option<PathBuf>),
    ResetRecordingDirectory,
    RecordingFinished(Result<(), String>),
    OpenButtonMenu(String),
    OpenButtonOptions(String),
    CloseButtonOptions,
    ButtonNameChanged {
        id: String,
        name: String,
    },
    OptionsChanged {
        generation: u64,
        action: button_options::Action,
    },
    OptionsAnalyzed {
        generation: u64,
        id: String,
        path: PathBuf,
        result: Result<std::sync::Arc<audio::waveform::Waveform>, String>,
    },
    DeleteButton(String),
    OpenAudio(String),
    AudioChosen {
        id: String,
        path: Option<PathBuf>,
    },
    BeginReorder(String),
    HoverReorder(String),
    LeaveReorder(String),
    EndReorder(window::Id),
    CancelReorder(window::Id),
    GridPointerMoved(Point),
    OpenGridMenu,
    DismissMenu,
    AddSound,
    PlayButton(String),
    Play,
    Pause,
    Resume,
    Stop,
    Saved(Report),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetChoice(Option<Target>);

impl std::fmt::Display for TargetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(target) => f.write_str(&target.display_label),
            None => f.write_str("No Output"),
        }
    }
}

impl Default for Soundboard {
    fn default() -> Self {
        Self::from_saved(raspiboard::storage::State::default(), None)
    }
}

impl Soundboard {
    pub fn theme(&self) -> Theme {
        self.theme.iced()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(audio::events).map(Message::Audio),
            self.hotkey_subscription(),
            Subscription::run(tray::events).map(Message::Tray),
            window::close_requests().map(Message::WindowCloseRequested),
            window::close_events().map(Message::WindowClosed),
            event::listen_with(|event, _status, window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::EscapePressed(window)),
                _ => reorder::window_event(&event, window),
            }),
        ])
    }

    fn send(&mut self, command: Command) {
        self.error = match &self.controller {
            Some(controller) => controller.send(command).err(),
            None => Some("The audio worker is starting. Please try again.".into()),
        };
    }

    fn send_volumes(&mut self) {
        if self.controller.is_some() {
            self.send(Command::SetVolumes {
                remote: self.remote_volume,
                local: self.local_volume,
            });
        }
    }

    fn transport_action(&self) -> Option<Message> {
        if !self.audio.connected || self.choosing_file.is_some() {
            return None;
        }
        match self.audio.playback {
            PlaybackStatus::Playing => Some(Message::Pause),
            PlaybackStatus::Paused => Some(Message::Resume),
            PlaybackStatus::Idle => self.can_play().then_some(Message::Play),
            PlaybackStatus::Loading => None,
        }
    }

    fn can_play(&self) -> bool {
        self.active_button
            .as_ref()
            .and_then(|id| self.buttons.iter().find(|button| &button.id == id))
            .and_then(|button| self.playback_for(button))
            .is_some()
    }

    fn playback_for<'a>(&'a self, button: &'a Button) -> Option<(&'a Path, Option<&'a Target>)> {
        if !self.audio.connected || self.choosing_file.is_some() {
            return None;
        }
        let duration = (self.options_button.as_deref() == Some(button.id.as_str()))
            .then(|| self.options.duration())
            .flatten();
        if playback_options(button).validate(duration).is_err() {
            return None;
        }
        Some((button.full_path.as_deref()?, self.audio.selected.as_ref()))
    }

    fn play_command(&self, id: &str) -> Option<Command> {
        let button = self.buttons.iter().find(|button| button.id == id)?;
        let (path, target) = self.playback_for(button)?;
        Some(Command::Play {
            button_id: id.to_owned(),
            overlap: button.overlap,
            path: path.to_path_buf(),
            target: target.cloned(),
            options: playback_options(button),
        })
    }

    fn play_button(&mut self, id: &str) {
        if self.reorder.is_some() {
            return;
        }
        if let Some(command) = self.play_command(id) {
            self.active_button = Some(id.to_owned());
            self.send(command);
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleRecording => {
                if let Some(command) = self.recording_command() {
                    self.send(command);
                }
            }
            Message::RecordingFormatChanged(format) => {
                self.recordings.format = format;
                return self.save_settings();
            }
            Message::ChooseRecordingDirectory => return self.choose_recording_directory(),
            Message::RecordingDirectoryChosen(directory) => {
                self.choosing_recording_directory = false;
                if let Some(directory) = directory {
                    self.recordings.directory = Some(directory);
                    return self.save_settings();
                }
            }
            Message::ResetRecordingDirectory => {
                self.recordings.directory = None;
                return self.save_settings();
            }
            Message::RecordingFinished(result) => {
                self.quitting = false;
                match result {
                    Ok(()) => return self.finish_quit(),
                    Err(error) => {
                        self.error = Some(error);
                        if self.main_window.is_none() {
                            return self.open_main();
                        }
                    }
                }
            }
            Message::Hotkeys(event) => return self.hotkey_event(event),
            Message::OpenSettings => {
                self.reorder = None;
                self.grid_menu = None;
                self.button_menu = None;
                return self.open_settings();
            }
            Message::CloseSettings => return self.close_settings(),
            Message::WindowCloseRequested(id) => return self.close_requested(id),
            Message::Tray(event) => return self.tray_event(event),
            Message::MinimizeToTrayChanged(enabled) => {
                self.minimize_to_tray = enabled;
                return self.save_settings();
            }
            Message::WindowClosed(id) => {
                if self.main_window == Some(id) {
                    return self.quit();
                }
                if self.settings_window == Some(id) {
                    self.settings_window = None;
                }
                if self.options_window == Some(id) {
                    self.options_window = None;
                    self.options_button = None;
                    self.options.cancel_analysis();
                }
            }
            Message::EscapePressed(id) => {
                if self.options_window == Some(id) {
                    return self.close_button_options();
                }
                if self.settings_window == Some(id) {
                    return self.close_settings();
                }
                if self.main_window == Some(id) {
                    self.reorder = None;
                    self.grid_menu = None;
                    self.button_menu = None;
                }
            }
            Message::BeginReorder(id) => self.begin_reorder(id),
            Message::HoverReorder(id) => self.hover_reorder(id),
            Message::LeaveReorder(id) => {
                if let Some(drag) = &mut self.reorder
                    && drag.target.as_deref() == Some(id.as_str())
                {
                    drag.target = None;
                }
            }
            Message::EndReorder(window) => {
                if self.main_window == Some(window) {
                    return self.finish_reorder();
                }
            }
            Message::CancelReorder(window) => {
                if self.main_window == Some(window) {
                    self.reorder = None;
                }
            }
            Message::GridPointerMoved(point) => self.grid_pointer = point,
            Message::OpenGridMenu => {
                if self.choosing_file.is_none() && self.reorder.is_none() {
                    self.grid_menu = Some(self.grid_pointer);
                    self.button_menu = None;
                }
            }
            Message::DismissMenu => {
                self.grid_menu = None;
                self.button_menu = None;
            }
            Message::AddSound => {
                self.grid_menu = None;
                self.button_menu = None;
                if self.choosing_file.is_none() {
                    let id = uuid::Uuid::now_v7().to_string();
                    return self.choose_audio(id);
                }
            }
            Message::ThemeChanged(theme) => {
                if self.theme != theme {
                    self.theme = theme;
                    return self.save_settings();
                }
            }
            Message::SoundButtonScaleChanged(scale) => {
                self.reorder = None;
                let scale = scale.clamp(MIN_SOUND_BUTTON_SCALE, MAX_SOUND_BUTTON_SCALE);
                if self.sound_button_scale != scale {
                    self.sound_button_scale = scale;
                    return self.save_settings();
                }
            }
            Message::RemoteVolumeChanged(volume) => {
                let previous = (self.remote_volume, self.local_volume);
                self.remote_volume = volume.min(100);
                if self.volumes_linked {
                    self.local_volume = self.remote_volume;
                }
                self.send_volumes();
                if previous != (self.remote_volume, self.local_volume) {
                    return self.save_settings();
                }
            }
            Message::LocalVolumeChanged(volume) => {
                let previous = (self.remote_volume, self.local_volume);
                self.local_volume = volume.min(100);
                if self.volumes_linked {
                    self.remote_volume = self.local_volume;
                }
                self.send_volumes();
                if previous != (self.remote_volume, self.local_volume) {
                    return self.save_settings();
                }
            }
            Message::ToggleVolumeLink => {
                self.volumes_linked = !self.volumes_linked;
                if self.volumes_linked {
                    self.local_volume = self.remote_volume;
                }
                self.send_volumes();
                return self.save_settings();
            }
            Message::Audio(Event::Ready(controller)) => {
                self.controller = Some(controller);
                self.send_volumes();
            }
            Message::Audio(Event::Changed(snapshot)) => self.audio = *snapshot,
            Message::SelectTarget(target) => {
                // Explicit None also cancels default selection before discovery finishes.
                if target.is_none() || self.audio.selected != target {
                    self.send(Command::Select(target));
                }
            }
            Message::SelectSource(source) => {
                if self.audio.source != source {
                    self.send(Command::SelectSource(source));
                }
            }
            Message::TogglePassthrough => {
                if let Some(command) = self.passthrough_command() {
                    self.send(command);
                }
            }
            Message::OpenButtonMenu(id) => {
                if self.reorder.is_none()
                    && self.choosing_file.is_none()
                    && self.buttons.iter().any(|button| button.id == id)
                {
                    self.grid_menu = Some(self.grid_pointer);
                    self.button_menu = Some(id);
                }
            }
            Message::OpenButtonOptions(id) => {
                self.reorder = None;
                self.grid_menu = None;
                self.button_menu = None;
                return self.open_button_options(id);
            }
            Message::CloseButtonOptions => return self.close_button_options(),
            Message::ButtonNameChanged { id, name } => return self.change_button_name(&id, name),
            Message::OptionsChanged { generation, action } => {
                return self.options_changed(generation, action);
            }
            Message::OptionsAnalyzed {
                generation,
                id,
                path,
                result,
            } => self.options_analyzed(generation, &id, &path, result),
            Message::DeleteButton(id) => return self.delete_button(&id),
            Message::OpenAudio(id) => {
                if self.choosing_file.is_some()
                    || !self.buttons.iter().any(|button| button.id == id)
                {
                    return Task::none();
                }
                self.grid_menu = None;
                self.button_menu = None;
                self.active_button = Some(id.clone());
                return self.choose_audio(id);
            }
            Message::AudioChosen { id, path } => {
                if self.choosing_file.as_deref() != Some(id.as_str()) {
                    return Task::none();
                }
                self.choosing_file = None;
                if let Some(path) = path {
                    if let Some(button) = self.buttons.iter_mut().find(|button| button.id == id) {
                        button.full_path = Some(path);
                        let saved = button.clone();
                        let save = self.save(Change::Button(saved));
                        if self.options_button.as_deref() == Some(id.as_str()) {
                            let analysis = self.reload_options();
                            return Task::batch([save, analysis]);
                        }
                        return save;
                    }
                    let button = Button {
                        id: id.clone(),
                        name: None,
                        full_path: Some(path),
                        sorting: self
                            .buttons
                            .iter()
                            .take(self.default_button_count)
                            .map(|button| button.sorting)
                            .max()
                            .unwrap_or(0)
                            .saturating_add(1),
                        ..Button::default()
                    };
                    self.button_categories
                        .insert(id.clone(), self.default_category_id.clone());
                    self.buttons
                        .insert(self.default_button_count, button.clone());
                    self.default_button_count += 1;
                    self.active_button = Some(id);
                    return self.save(Change::AddButton(button));
                }
            }
            Message::PlayButton(id) => self.play_button(&id),
            Message::Play => {
                if let Some(id) = self.active_button.clone() {
                    self.play_button(&id);
                }
            }
            Message::Pause => self.send(Command::Pause),
            Message::Resume => self.send(Command::Resume),
            Message::Stop => self.send(Command::Stop),
            Message::Saved(report) => self.saved(report),
        }
        Task::none()
    }

    fn choose_audio(&mut self, id: String) -> Task<Message> {
        self.reorder = None;
        if self.controller.is_some() {
            self.send(Command::Stop);
        }
        self.choosing_file = Some(id.clone());
        Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Choose a sound")
                    .add_filter(
                        "Audio",
                        &[
                            "wav", "wave", "flac", "mp1", "mp2", "mp3", "mpa", "aac", "m4a", "m4b",
                            "mp4", "ogg", "oga", "aif", "aiff", "aifc", "caf", "mkv", "mka",
                            "webm", "opus",
                        ],
                    )
                    .add_filter("All files", &["*"])
                    .pick_file()
                    .await
                    .map(|file| file.path().to_path_buf())
            },
            move |path| Message::AudioChosen {
                id: id.clone(),
                path,
            },
        )
    }

    pub fn view(&self, window: window::Id) -> Element<'_, Message> {
        if self.settings_window == Some(window) {
            return settings::view(
                self.theme,
                self.sound_button_scale,
                self.minimize_to_tray,
                &self.recordings,
                self.recording_directory(),
                self.choosing_recording_directory,
            );
        }
        if self.options_window == Some(window) {
            return self.options_view();
        }
        self.soundboard_view()
    }

    fn soundboard_view(&self) -> Element<'_, Message> {
        let sounds = scrollable(
            container(
                grid(self.buttons.iter().map(|button| self.sound_button(button)))
                    .fluid(self.button_dimensions().width)
                    .height(iced::Shrink)
                    .spacing(16),
            )
            .padding(iced::Padding {
                top: CONTENT_PADDING + 8.0,
                right: CONTENT_PADDING + 8.0,
                bottom: 8.0,
                left: CONTENT_PADDING + 8.0,
            }),
        )
        .spacing(0)
        .width(Fill)
        .height(Fill);
        let sounds = grid_menu::area(sounds.into());
        let action = self.transport_action();
        let can_play = action.is_some();
        let (transport_icon, transport_tone) = if self.audio.playback == PlaybackStatus::Playing {
            (Icon::Pause, sound_pad::Tone::Pause)
        } else {
            (Icon::Play, sound_pad::Tone::Play)
        };
        let can_stop = self.audio.playback != PlaybackStatus::Idle;
        let transport = row![
            sound_pad::raised(
                button(icon(transport_icon).size(28))
                    .width(64)
                    .height(64)
                    .on_press_maybe(action),
                can_play,
                false,
                transport_tone,
                false,
            ),
            sound_pad::raised(
                button(icon(Icon::Stop).size(28))
                    .width(64)
                    .height(64)
                    .on_press_maybe(can_stop.then_some(Message::Stop)),
                can_stop,
                false,
                sound_pad::Tone::Stop,
                false,
            ),
            self.record_button(),
        ]
        .spacing(10);
        let footer = row![
            transport,
            self.volume_controls(),
            space::horizontal(),
            self.routing_controls(),
            sound_pad::animated(
                button(icon(Icon::Settings).size(28))
                    .width(64)
                    .height(64)
                    .padding(8)
                    .on_press(Message::OpenSettings),
                true,
                false,
            ),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let mut content = column![sounds].spacing(12);
        if let Some(status) = self.recording_status_text() {
            content = content.push(
                container(text(status).size(12).style(text::secondary))
                    .padding([0.0, CONTENT_PADDING]),
            );
        }
        for error in [
            self.storage_error.as_ref(),
            self.error.as_ref().or(self.audio.error.as_ref()),
        ]
        .into_iter()
        .flatten()
        {
            content = content.push(
                container(text(error).size(14).style(text::danger)).padding([0.0, CONTENT_PADDING]),
            );
        }
        let footer = container(footer).padding(12).style(|theme: &Theme| {
            let colors = raspiboard::theme::tokens(theme);
            container::Style {
                background: Some(colors.surface.into()),
                border: iced::Border {
                    color: colors.border,
                    width: 1.0,
                    radius: colors.radius.into(),
                },
                ..container::Style::default()
            }
        });
        let footer = container(footer).padding(iced::Padding {
            top: 0.0,
            right: CONTENT_PADDING,
            bottom: CONTENT_PADDING,
            left: CONTENT_PADDING,
        });
        let content = content.push(footer).width(Fill).height(Fill);
        grid_menu::view(content.into(), self.grid_menu, self.button_menu.as_deref())
    }

    fn button_playing(&self, id: &str) -> bool {
        self.audio
            .playbacks
            .iter()
            .any(|instance| instance.button_id == id && instance.status == PlaybackStatus::Playing)
    }

    fn sound_button<'a>(&'a self, record: &'a Button) -> Element<'a, Message> {
        let label = if self.choosing_file.as_deref() == Some(record.id.as_str()) {
            "Choosing sound…"
        } else {
            record.display_name()
        };
        let playing = self.button_playing(&record.id);
        let enabled = self.playback_for(record).is_some();
        let sound = button(
            container(fitted_label::label(label))
                .width(Fill)
                .center_y(Fill)
                .clip(true),
        )
        .width(Fill)
        .height(self.button_dimensions().height)
        .padding(12)
        .on_press_maybe(enabled.then(|| Message::PlayButton(record.id.clone())));
        reorder::pad(
            sound_pad::colored(
                sound,
                enabled,
                playing,
                record.color.as_deref().and_then(sound_pad::parse_color),
                record.overlap,
            ),
            &record.id,
            self.reorder
                .as_ref()
                .is_some_and(|drag| drag.source == record.id),
            self.reorder_edge(&record.id),
        )
    }

    fn button_dimensions(&self) -> iced::Size {
        let scale = f32::from(self.sound_button_scale) / 100.0;
        iced::Size::new(180.0 * scale, 86.0 * scale)
    }

    fn volume_controls(&self) -> Element<'_, Message> {
        let levels = column![
            volume_row(
                "Remote Volume",
                self.remote_volume,
                Message::RemoteVolumeChanged,
                self.audio.selected.is_some(),
            ),
            volume_row(
                "Local Volume",
                self.local_volume,
                Message::LocalVolumeChanged,
                true,
            ),
        ]
        .spacing(10);
        let linked = self.volumes_linked;
        let link = button(icon(if linked { Icon::Link } else { Icon::Unlink }).size(16))
            .width(24)
            .height(24)
            .padding(2)
            .on_press(Message::ToggleVolumeLink)
            .style(move |theme: &Theme, status| {
                let palette = theme.extended_palette();
                button::Style {
                    text_color: if linked
                        || matches!(status, button::Status::Hovered | button::Status::Pressed)
                    {
                        palette.primary.strong.color
                    } else {
                        palette.background.base.text
                    },
                    ..button::Style::default()
                }
            });
        let stroke = |width, height| {
            container(space())
                .width(width)
                .height(height)
                .style(move |theme: &Theme| {
                    let palette = theme.extended_palette();
                    let color = if linked {
                        palette.primary.strong.color
                    } else {
                        palette.background.base.text.scale_alpha(0.6)
                    };
                    container::Style {
                        background: Some(color.into()),
                        ..container::Style::default()
                    }
                })
        };
        let upper = column![stroke(14, 2), stroke(2, 4)].align_x(iced::Right);
        let lower = column![stroke(2, 4), stroke(14, 2)].align_x(iced::Right);
        let link = column![upper, link, lower];
        row![levels, link].spacing(4).align_y(Center).into()
    }

    fn target_controls(&self) -> Element<'_, Message> {
        let choices = std::iter::once(TargetChoice(None))
            .chain(
                self.audio
                    .targets
                    .iter()
                    .cloned()
                    .map(|target| TargetChoice(Some(target))),
            )
            .collect::<Vec<_>>();
        pick_list(
            choices,
            Some(TargetChoice(self.audio.selected.clone())),
            |choice| Message::SelectTarget(choice.0),
        )
        .text_size(12)
        .padding(6)
        .width(220)
        .into()
    }
}

fn volume_row(
    label: &'static str,
    value: u8,
    on_change: fn(u8) -> Message,
    enabled: bool,
) -> Element<'static, Message> {
    let text_style = move |theme: &Theme| {
        let mut color = theme.palette().text;
        if !enabled {
            color.a *= 0.5;
        }
        text::Style { color: Some(color) }
    };
    row![
        text(label).size(14).width(120).style(text_style),
        text(format!("{value}%"))
            .size(13)
            .width(36)
            .align_x(iced::Right)
            .style(text_style),
        volume_slider::view(value, on_change, enabled),
    ]
    .spacing(8)
    .align_y(Center)
    .into()
}

#[cfg(test)]
mod tests;

fn playback_options(button: &Button) -> audio::PlaybackOptions {
    audio::PlaybackOptions {
        start_ms: button.start_ms,
        end_ms: button.end_ms,
        relative_volume: button.relative_volume,
    }
}
