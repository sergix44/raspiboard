use super::*;
use raspiboard::storage::{Change, Report, Settings, State, Store, Writer};

impl Soundboard {
    pub fn boot() -> Self {
        match Store::in_home() {
            Ok(store) => Self::from_store(store),
            Err(error) => Self {
                storage_error: Some(error),
                ..Self::default()
            },
        }
    }

    pub(super) fn from_store(store: Store) -> Self {
        let directory = store.recordings_directory();
        let mut board = match store.load_or_create() {
            Ok(saved) => match Writer::start(store) {
                Ok(writer) => Self::from_saved(saved, Some(writer)),
                Err(error) => Self {
                    storage_error: Some(error),
                    ..Self::from_saved(saved, None)
                },
            },
            Err(error) => Self {
                storage_error: Some(error),
                ..Self::default()
            },
        };
        board.default_recording_directory = Some(directory);
        board
    }

    pub(super) fn from_saved(saved: State, persistence: Option<Writer>) -> Self {
        let buttons = saved.ordered_buttons();
        let default_button_count = saved
            .categories
            .iter()
            .find(|category| category.category_name.is_none())
            .map_or(0, |category| category.entries.len());
        let default_category_id = saved
            .categories
            .iter()
            .find(|category| category.category_name.is_none())
            .map(|category| category.id.clone())
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let button_categories = saved
            .categories
            .iter()
            .flat_map(|category| {
                category
                    .entries
                    .iter()
                    .map(move |button| (button.id.clone(), category.id.clone()))
            })
            .collect();
        let mut board = Self {
            default_category_id,
            button_categories,
            reorder: None,
            theme: saved.settings.theme,
            main_window: None,
            main_hidden: false,
            tray_available: false,
            minimize_to_tray: saved.settings.minimize_to_tray,
            recordings: saved.settings.recordings,
            default_recording_directory: Store::in_home()
                .ok()
                .map(|store| store.recordings_directory()),
            choosing_recording_directory: false,
            quitting: false,
            settings_window: None,
            options_window: None,
            options_button: None,
            options: options_state::OptionsState::default(),
            hotkeys: hotkeys::State::default(),
            remote_volume: saved.settings.remote_volume,
            local_volume: saved.settings.local_volume,
            volumes_linked: saved.settings.volumes_linked,
            sound_button_scale: saved.settings.sound_button_scale,
            controller: None,
            audio: Snapshot::default(),
            active_button: buttons.first().map(|button| button.id.clone()),
            buttons,
            default_button_count,
            choosing_file: None,
            grid_pointer: Point::ORIGIN,
            grid_menu: None,
            button_menu: None,
            error: None,
            persistence,
            storage_error: None,
            last_saved_revision: 0,
        };
        board.sync_hotkeys(None);
        board
    }

    pub(super) fn save_settings(&mut self) -> Task<Message> {
        self.save(Change::Settings(Settings {
            theme: self.theme,
            remote_volume: self.remote_volume,
            local_volume: self.local_volume,
            volumes_linked: self.volumes_linked,
            sound_button_scale: self.sound_button_scale,
            minimize_to_tray: self.minimize_to_tray,
            recordings: self.recordings.clone(),
        }))
    }

    pub(super) fn save(&mut self, change: Change) -> Task<Message> {
        match self.persistence.as_mut() {
            Some(writer) => Task::perform(writer.queue(change), Message::Saved),
            None => Task::none(),
        }
    }

    pub(super) fn saved(&mut self, report: Report) {
        if report.revision >= self.last_saved_revision {
            self.last_saved_revision = report.revision;
            self.storage_error = (!report.errors.is_empty()).then(|| report.errors.join("\n"));
        }
    }
}

#[cfg(test)]
mod tests;
