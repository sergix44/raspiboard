use super::{
    Message, Soundboard,
    icons::{Icon, icon},
    options_state::{crop_values, format_time},
    sound_pad, waveform, windows,
};
use iced::widget::{
    button, checkbox, column, container, row, rule, scrollable, space, stack, text, text_input,
};
use iced::{Color, Element, Fill, Size, Task, window};
use raspiboard::{audio, storage::Change};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Debug, Clone)]
pub enum Action {
    Preview,
    StopPreview,
    ShowColor,
    CancelColor,
    ColorPicked(Color),
    ResetColor,
    PrefillFilename,
    Gain(f32),
    Overlap(bool),
    Start(String),
    End(String),
    ResetCrop,
    SetHotkey,
    ClearHotkey,
    Range { start: u64, end: u64, commit: bool },
}

impl Soundboard {
    pub(super) fn open_button_options(&mut self, id: String) -> Task<Message> {
        if !self.buttons.iter().any(|button| button.id == id) {
            return Task::none();
        }
        let changed = self.options_button.as_ref() != Some(&id) || self.options_window.is_none();
        self.options_button = Some(id);
        let analysis = if changed {
            self.reload_options()
        } else {
            Task::none()
        };
        if let Some(window) = self.options_window {
            return Task::batch([
                analysis,
                window::minimize(window, false).chain(window::gain_focus(window)),
            ]);
        }
        let (id, open) = window::open(window::Settings {
            size: Size::new(760.0, 760.0),
            min_size: Some(Size::new(560.0, 480.0)),
            position: window::Position::Centered,
            ..windows::window_settings()
        });
        self.options_window = Some(id);
        Task::batch([analysis, open.discard()])
    }

    pub(super) fn close_button_options(&mut self) -> Task<Message> {
        self.options.cancel_analysis();
        self.options.color_picker = false;
        self.options_window
            .map(window::close)
            .unwrap_or_else(Task::none)
    }

    pub(super) fn reload_options(&mut self) -> Task<Message> {
        let Some(button) = self
            .buttons
            .iter()
            .find(|b| Some(b.id.as_str()) == self.options_button.as_deref())
        else {
            return Task::none();
        };
        self.options.reset(button);
        let Some(path) = button.full_path.clone() else {
            return Task::none();
        };
        let id = button.id.clone();
        let generation = self.options.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.options.cancel = Some(cancel.clone());
        self.options.loading = true;
        let (sender, receiver) = async_channel::bounded(1);
        let source = path.clone();
        let worker = std::thread::Builder::new()
            .name("waveform-analysis".into())
            .spawn(move || {
                let result = audio::waveform::analyze(&source, &cancel);
                let _ = sender.send_blocking(result);
            });
        if let Err(error) = worker {
            log::error!(target: "raspiboard::audio", "Cannot start waveform worker: {error}");
            self.options.loading = false;
            self.options.analysis_error = Some(format!("Cannot load waveform: {error}"));
            return Task::none();
        }
        Task::perform(
            async move {
                receiver
                    .recv()
                    .await
                    .unwrap_or_else(|_| Err("Waveform worker stopped.".into()))
            },
            move |result| Message::OptionsAnalyzed {
                generation,
                id: id.clone(),
                path: path.clone(),
                result,
            },
        )
    }

    pub(super) fn options_analyzed(
        &mut self,
        generation: u64,
        id: &str,
        path: &Path,
        result: Result<Arc<audio::waveform::Waveform>, String>,
    ) {
        if generation != self.options.generation
            || self.options_button.as_deref() != Some(id)
            || !self
                .buttons
                .iter()
                .any(|b| b.id == id && b.full_path.as_deref() == Some(path))
        {
            return;
        }
        self.options.loading = false;
        match result {
            Ok(waveform) => {
                self.options.waveform = Some(waveform);
                self.options.analysis_error = None;
                self.options.crop_error = crop_values(
                    &self.options.start,
                    &self.options.end,
                    self.options.duration(),
                )
                .err();
            }
            Err(error) => {
                log::error!(target: "raspiboard::audio", "Cannot load waveform: {error}");
                self.options.analysis_error = Some(error);
            }
        }
    }

    pub(super) fn preview_playing(&self) -> bool {
        self.audio.playbacks.iter().any(|instance| {
            instance.preview && self.options_button.as_deref() == Some(instance.button_id.as_str())
        })
    }

    pub(super) fn preview_command(&self) -> Option<audio::Command> {
        if !self.audio.connected || self.choosing_file.is_some() {
            return None;
        }
        let record = self
            .buttons
            .iter()
            .find(|b| Some(b.id.as_str()) == self.options_button.as_deref())?;
        let (start, end) = crop_values(
            &self.options.start,
            &self.options.end,
            self.options.duration(),
        )
        .ok()?;
        let options = audio::PlaybackOptions {
            start_ms: start,
            end_ms: end,
            relative_volume: record.relative_volume,
        };
        options.validate(self.options.duration()).ok()?;
        Some(audio::Command::Preview {
            button_id: record.id.clone(),
            path: record.full_path.clone()?,
            options,
        })
    }

    pub(super) fn options_changed(&mut self, generation: u64, action: Action) -> Task<Message> {
        if generation != self.options.generation || self.options_window.is_none() {
            return Task::none();
        }
        let Some(id) = self.options_button.clone() else {
            return Task::none();
        };
        let Some(index) = self.buttons.iter().position(|b| b.id == id) else {
            return Task::none();
        };
        match action {
            Action::PrefillFilename => {
                let name = self.buttons[index]
                    .full_path
                    .as_deref()
                    .and_then(Path::file_stem)
                    .and_then(|name| name.to_str())
                    .map(str::to_owned);
                return name.map_or_else(Task::none, |name| self.change_button_name(&id, name));
            }
            Action::SetHotkey => {
                self.sync_hotkeys(Some(id));
                return Task::none();
            }
            Action::ClearHotkey => {
                self.buttons[index].hotkey = None;
                self.sync_hotkeys(None);
            }
            Action::Preview => {
                if let Some(command) = self.preview_command() {
                    self.active_button = Some(id);
                    self.send(command);
                }
                return Task::none();
            }
            Action::StopPreview => {
                if self.preview_playing() {
                    self.send(audio::Command::Stop);
                }
                return Task::none();
            }
            Action::ShowColor => {
                self.options.color_picker = true;
                return Task::none();
            }
            Action::CancelColor => {
                self.options.color_picker = false;
                return Task::none();
            }
            Action::ColorPicked(color) => {
                self.options.color_picker = false;
                self.buttons[index].color = Some(sound_pad::color_hex(color));
            }
            Action::ResetColor => {
                self.options.color_picker = false;
                self.buttons[index].color = None;
            }
            Action::Overlap(enabled) => {
                if self.buttons[index].overlap == enabled {
                    return Task::none();
                }
                self.buttons[index].overlap = enabled;
            }
            Action::Gain(db) => {
                if !db.is_finite() || !(-15.0..=15.0).contains(&db) {
                    return Task::none();
                }
                let volume = (db != 0.0).then_some(db);
                if self.buttons[index].relative_volume == volume {
                    return Task::none();
                }
                self.buttons[index].relative_volume = volume;
            }
            action => {
                let mut commit = true;
                match action {
                    Action::Start(value) => self.options.start = value,
                    Action::End(value) => self.options.end = value,
                    Action::ResetCrop => {
                        self.options.start.clear();
                        self.options.end.clear();
                    }
                    Action::Range {
                        start,
                        end,
                        commit: save,
                    } => {
                        self.options.start = format_time((start != 0).then_some(start));
                        self.options.end =
                            format_time((Some(end) != self.options.duration()).then_some(end));
                        commit = save;
                    }
                    _ => unreachable!(),
                }
                let values = crop_values(
                    &self.options.start,
                    &self.options.end,
                    self.options.duration(),
                );
                self.options.crop_error = values.as_ref().err().cloned();
                match values {
                    Ok((start, end)) if commit => {
                        if self.buttons[index].start_ms == start
                            && self.buttons[index].end_ms == end
                        {
                            return Task::none();
                        }
                        self.buttons[index].start_ms = start;
                        self.buttons[index].end_ms = end;
                    }
                    _ => return Task::none(),
                }
            }
        }
        self.save(Change::Button(self.buttons[index].clone()))
    }

    pub(super) fn change_button_name(&mut self, id: &str, name: String) -> Task<Message> {
        if self.options_button.as_deref() != Some(id) {
            return Task::none();
        }
        let Some(button) = self.buttons.iter_mut().find(|button| button.id == id) else {
            return Task::none();
        };
        let name = (!name.trim().is_empty()).then_some(name);
        if button.name == name {
            return Task::none();
        }
        button.name = name;
        let saved = button.clone();
        self.save(Change::Button(saved))
    }

    pub(super) fn delete_button(&mut self, id: &str) -> Task<Message> {
        self.reorder = None;
        self.grid_menu = None;
        self.button_menu = None;
        let Some(index) = self.buttons.iter().position(|button| button.id == id) else {
            return Task::none();
        };
        self.buttons.remove(index);
        if self
            .hotkeys
            .config
            .bindings
            .iter()
            .any(|binding| binding.id == id)
        {
            self.sync_hotkeys(None);
        }
        self.button_categories.remove(id);
        if index < self.default_button_count {
            self.default_button_count -= 1;
        }
        let selected = self.active_button.as_deref() == Some(id);
        let playing = self
            .audio
            .playbacks
            .iter()
            .any(|instance| instance.button_id == id);
        if playing || (selected && self.controller.is_some()) {
            self.send(audio::Command::Stop);
        }
        if selected {
            self.active_button = None;
        }
        if self.choosing_file.as_deref() == Some(id) {
            self.choosing_file = None;
        }
        let close = if self.options_button.as_deref() == Some(id) {
            self.options_button = None;
            self.options.cancel_analysis();
            self.options_window
                .take()
                .map(window::close)
                .unwrap_or_else(Task::none)
        } else {
            Task::none()
        };
        Task::batch([close, self.save(Change::DeleteButton(id.into()))])
    }

    pub(super) fn options_view(&self) -> Element<'_, Message> {
        let Some(record) = self
            .buttons
            .iter()
            .find(|b| Some(b.id.as_str()) == self.options_button.as_deref())
        else {
            return text("This sound was deleted.").into();
        };
        let id = record.id.clone();
        let clear_id = id.clone();
        let generation = self.options.generation;
        let event = move |action| Message::OptionsChanged { generation, action };
        let selected_color = record
            .color
            .as_deref()
            .and_then(sound_pad::parse_color)
            .unwrap_or_else(|| self.theme.iced().extended_palette().primary.base.color);
        let swatch = button(space().width(18).height(18))
            .width(40)
            .height(40)
            .on_press(event(Action::ShowColor))
            .style(move |_, status| button::Style {
                background: Some(selected_color.into()),
                border: iced::Border {
                    width: 1.0,
                    color: if matches!(status, button::Status::Hovered) {
                        Color::WHITE
                    } else {
                        selected_color
                    },
                    radius: 6.0.into(),
                },
                ..button::Style::default()
            });
        let picker = iced_aw::ColorPicker::new(
            self.options.color_picker,
            selected_color,
            swatch,
            event(Action::CancelColor),
            move |color| event(Action::ColorPicked(color)),
        )
        .actions(
            sound_pad::animated(
                button(text("Cancel").width(Fill).center())
                    .on_press(event(Action::CancelColor))
                    .width(Fill)
                    .height(40)
                    .padding(8),
                true,
                false,
            ),
            sound_pad::animated(
                button(text("Apply").width(Fill).center())
                    .on_press(event(Action::CancelColor))
                    .width(Fill)
                    .height(40)
                    .padding(8),
                true,
                false,
            ),
        );
        let name = stack![
            text_input(record.display_name(), record.name.as_deref().unwrap_or(""))
                .on_input(move |name| Message::ButtonNameChanged {
                    id: id.clone(),
                    name
                })
                .size(16)
                .line_height(iced::Pixels(20.0))
                .padding(iced::Padding {
                    right: 42.0,
                    ..iced::Padding::new(10.0)
                }),
            container(
                button(text("×").size(20))
                    .on_press(Message::ButtonNameChanged {
                        id: clear_id,
                        name: String::new()
                    })
                    .width(28)
                    .height(28)
                    .style(button::text)
            )
            .align_right(Fill)
            .center_y(Fill)
            .padding(6),
        ];
        let path = record
            .full_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "No sound assigned".into());
        let db = record.relative_volume.unwrap_or(0.0);
        let can_prefill = record
            .full_path
            .as_deref()
            .and_then(Path::file_stem)
            .and_then(|name| name.to_str())
            .is_some();
        let color = column![
            space().height(24),
            picker,
            button(text("Reset").size(10).width(Fill).center())
                .on_press(event(Action::ResetColor))
                .width(40)
                .height(20)
                .padding(0)
                .style(button::text),
        ]
        .spacing(8)
        .width(40);
        let name = column![
            row![
                text("Name").size(14),
                space::horizontal(),
                button(text("Prefill filename").size(12))
                    .on_press_maybe(can_prefill.then(|| event(Action::PrefillFilename)))
                    .height(24)
                    .padding([0, 4])
                    .style(button::text),
            ]
            .height(24)
            .align_y(iced::Center),
            name,
            text("Leave empty to use the filename.")
                .size(12)
                .style(text::secondary),
        ]
        .spacing(8)
        .width(Fill);
        let mut controls = column![
            row![color, name].spacing(12).align_y(iced::Top),
            text(format!("Path: {path}"))
                .size(13)
                .wrapping(text::Wrapping::Glyph),
            space().height(8),
            row![
                text("Relative Volume").size(14),
                space::horizontal(),
                text(format!("{db:+.1} dB")).size(14)
            ],
            super::gain_slider::view(db, move |db| event(Action::Gain(db))),
            row![
                text("−15 dB").size(12),
                space::horizontal(),
                text("0 dB").size(12),
                space::horizontal(),
                text("+15 dB").size(12)
            ],
            text("Volume modifier relative to the Remote and Local volumes.")
                .size(12)
                .style(text::secondary),
            space().height(8),
            checkbox(record.overlap)
                .label("Overlap Mode")
                .on_toggle(move |enabled| event(Action::Overlap(enabled))),
            text("Allow this sound to overlap itself or other sounds being played.")
                .size(12)
                .style(text::secondary),
            space().height(8),
            rule::horizontal(1),
            text("Crop").size(14),
            column![
                row![
                    super::crop_input::view(
                        &self.options.start,
                        0,
                        self.options.duration(),
                        move |value| event(Action::Start(value))
                    ),
                    icon(Icon::ArrowRight).size(24).width(24).height(40),
                    super::crop_input::view(
                        &self.options.end,
                        self.options.duration().unwrap_or(0),
                        self.options.duration(),
                        move |value| event(Action::End(value))
                    ),
                    button(text("×").size(20))
                        .on_press(event(Action::ResetCrop))
                        .width(32)
                        .height(36)
                        .style(button::text),
                ]
                .spacing(12)
                .align_y(iced::Center),
                row![
                    text("Start (seconds)").size(12).width(Fill),
                    space().width(24),
                    text("End (seconds)").size(12).width(Fill),
                    space().width(32),
                ]
                .spacing(12),
            ]
            .spacing(6),
        ]
        .spacing(12);
        if let Some(error) = &self.options.crop_error {
            controls = controls.push(text(error).size(12).style(text::danger));
        }
        if let Some(wave) = &self.options.waveform {
            let (start, end) = crop_values(
                &self.options.start,
                &self.options.end,
                Some(wave.duration_ms),
            )
            .unwrap_or((record.start_ms, record.end_ms));
            let chart = waveform::view(wave, start, end, move |start, end, commit| {
                event(Action::Range { start, end, commit })
            });
            controls = controls.push(iced::widget::keyed::Column::with_children([(
                generation, chart,
            )]));
            controls = controls.push(
                text(format!(
                    "Duration: {} seconds",
                    format_time(Some(wave.duration_ms))
                ))
                .size(12)
                .style(text::secondary),
            );
            controls = controls.push(text("Drag to select a range, drag either edge to adjust it, or click to move the nearest edge.").size(12).style(text::secondary));
        } else if self.options.loading {
            controls = controls.push(text("Loading waveform…").size(13).style(text::secondary));
        } else if let Some(error) = &self.options.analysis_error {
            controls = controls.push(text(error).size(12).style(text::danger));
        } else {
            controls = controls.push(
                text("Assign a sound to see its waveform.")
                    .size(13)
                    .style(text::secondary),
            );
        }

        controls = controls.push(rule::horizontal(1));
        controls = controls.push(text("Hotkey").size(14));
        let hotkey = record
            .hotkey
            .as_deref()
            .filter(|key| !key.trim().is_empty());
        let pending = self.hotkeys.pending;
        let key = button(text(hotkey.unwrap_or("No hotkey set")).size(14))
            .width(Fill)
            .height(40)
            .padding(10)
            .style(button::secondary)
            .on_press_maybe((!pending).then(|| event(Action::SetHotkey)));
        controls = controls.push(
            row![
                key,
                sound_pad::animated(
                    button(
                        text(if hotkey.is_some() { "× Clear" } else { "Set" })
                            .width(Fill)
                            .center()
                    )
                    .width(100)
                    .height(40)
                    .padding(8)
                    .on_press_maybe((hotkey.is_some() || !pending).then(|| event(
                        if hotkey.is_some() {
                            Action::ClearHotkey
                        } else {
                            Action::SetHotkey
                        }
                    ))),
                    hotkey.is_some() || !pending,
                    false,
                )
            ]
            .spacing(12)
            .align_y(iced::Center),
        );
        controls = controls.push(
            text(if pending {
                "Waiting for the desktop shortcut chooser…"
            } else {
                "Works while the app is minimized. Click the shortcut to change it."
            })
            .size(12)
            .style(text::secondary),
        );
        if let Some(error) = &self.hotkeys.error {
            controls = controls.push(text(error).size(12).style(text::danger));
        }

        if let Some(error) = &self.storage_error {
            controls = controls.push(text(error).size(12).style(text::danger));
        }
        let previewing = self.preview_playing();
        let preview_enabled = previewing || self.preview_command().is_some();
        let preview = sound_pad::raised(
            button(
                row![
                    icon(if previewing { Icon::Stop } else { Icon::Play })
                        .size(18)
                        .width(20),
                    text(if previewing { "Stop" } else { "Preview" }).size(14)
                ]
                .spacing(8)
                .align_y(iced::Center),
            )
            .width(164)
            .height(40)
            .padding([8, 12])
            .on_press_maybe(preview_enabled.then(|| {
                event(if previewing {
                    Action::StopPreview
                } else {
                    Action::Preview
                })
            })),
            preview_enabled,
            false,
            if previewing {
                sound_pad::Tone::Stop
            } else {
                sound_pad::Tone::Play
            },
            false,
        );
        column![
            scrollable(container(controls).padding(iced::Padding {
                top: 24.0,
                right: 24.0,
                bottom: 0.0,
                left: 24.0,
            }))
            .spacing(0)
            .height(Fill),
            container(row![
                preview,
                space::horizontal(),
                sound_pad::animated(
                    button(text("Close").width(Fill).center())
                        .on_press(Message::CloseButtonOptions)
                        .width(84)
                        .height(36)
                        .padding(8),
                    true,
                    false
                )
            ])
            .padding(iced::Padding {
                top: 20.0,
                right: 24.0,
                bottom: 24.0,
                left: 24.0,
            })
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }
}
