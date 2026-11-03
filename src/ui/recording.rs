use super::{
    Message, Soundboard,
    icons::{Icon, icon},
    sound_pad,
};
use iced::widget::{button, column, pick_list, row, text};
use iced::{Element, Fill, Task};
use raspiboard::{
    audio::{Command, RecordingStatus},
    recording::{RecordingFormat, RecordingSettings},
};
use std::path::PathBuf;

impl Soundboard {
    pub(super) fn recording_directory(&self) -> Option<PathBuf> {
        self.recordings
            .directory
            .clone()
            .or_else(|| self.default_recording_directory.clone())
    }

    pub(super) fn recording_command(&self) -> Option<Command> {
        if self.quitting {
            return None;
        }
        match self.audio.recording {
            RecordingStatus::Starting | RecordingStatus::Recording => Some(Command::StopRecording),
            RecordingStatus::Idle if self.audio.connected => {
                self.recording_directory()
                    .map(|directory| Command::StartRecording {
                        directory,
                        format: self.recordings.format,
                    })
            }
            _ => None,
        }
    }

    pub(super) fn record_button(&self) -> Element<'_, Message> {
        let enabled = self.recording_command().is_some();
        sound_pad::blinking(
            button(icon(Icon::Record).size(28))
                .width(64)
                .height(64)
                .on_press_maybe(enabled.then_some(Message::ToggleRecording)),
            enabled,
            self.audio.recording == RecordingStatus::Recording,
            self.theme().extended_palette().danger.base.color,
        )
    }

    pub(super) fn recording_status_text(&self) -> Option<String> {
        if self.quitting {
            return Some("Finishing recording before quitting…".into());
        }
        match self.audio.recording {
            RecordingStatus::Starting => Some("Starting recording…".into()),
            RecordingStatus::Saving => Some("Saving recording…".into()),
            RecordingStatus::Idle => self
                .audio
                .last_recording
                .as_ref()
                .map(|path| format!("Saved recording: {}", path.display())),
            RecordingStatus::Recording => None,
        }
    }

    pub(super) fn choose_recording_directory(&mut self) -> Task<Message> {
        if self.choosing_recording_directory {
            return Task::none();
        }
        self.choosing_recording_directory = true;
        let directory = self.recording_directory();
        Task::perform(
            async move {
                let mut dialog = rfd::AsyncFileDialog::new().set_title("Choose recordings folder");
                if let Some(directory) = directory {
                    dialog = dialog.set_directory(directory);
                }
                dialog
                    .pick_folder()
                    .await
                    .map(|folder| folder.path().to_path_buf())
            },
            Message::RecordingDirectoryChosen,
        )
    }
}

pub(super) fn settings_view(
    settings: &RecordingSettings,
    directory: Option<PathBuf>,
    choosing: bool,
) -> Element<'static, Message> {
    let path = directory
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Home directory unavailable".into());
    column![
        text("Recordings").size(18),
        text("Format").size(14),
        pick_list(
            RecordingFormat::ALL,
            Some(settings.format),
            Message::RecordingFormatChanged
        )
        .padding(12)
        .width(Fill),
        text("Output folder").size(14),
        text(path).size(12),
        row![
            button(text(if choosing {
                "Choosing…"
            } else {
                "Choose folder"
            }))
            .padding(8)
            .on_press_maybe((!choosing).then_some(Message::ChooseRecordingDirectory)),
            button(text("Use default folder"))
                .padding(8)
                .on_press_maybe(
                    settings
                        .directory
                        .is_some()
                        .then_some(Message::ResetRecordingDirectory)
                ),
        ]
        .spacing(8),
        text("Saved automatically. Changes apply to the next recording.")
            .size(13)
            .style(text::secondary),
    ]
    .spacing(8)
    .into()
}
