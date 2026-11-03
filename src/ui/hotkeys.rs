use super::{Message, Soundboard};
use iced::{Subscription, Task};
use raspiboard::{
    hotkeys::{self, Binding, Config, Event},
    storage::Change,
};

#[derive(Default)]
pub(super) struct State {
    pub config: Config,
    pub pending: bool,
    pub error: Option<String>,
}

impl Soundboard {
    pub(super) fn sync_hotkeys(&mut self, configure: Option<String>) {
        let bindings: Vec<_> = self
            .buttons
            .iter()
            .filter(|button| {
                button
                    .hotkey
                    .as_ref()
                    .is_some_and(|key| !key.trim().is_empty())
                    || configure.as_deref() == Some(button.id.as_str())
            })
            .map(|button| Binding {
                id: button.id.clone(),
                description: format!("Play {}", button.display_name()),
            })
            .collect();
        self.hotkeys.pending = !bindings.is_empty();
        self.hotkeys.error = None;
        self.hotkeys.config = Config {
            revision: self.hotkeys.config.revision.wrapping_add(1),
            bindings,
            configure,
        };
    }

    pub(super) fn hotkey_subscription(&self) -> Subscription<Message> {
        if self.hotkeys.config.bindings.is_empty() {
            Subscription::none()
        } else {
            Subscription::run_with(self.hotkeys.config.clone(), hotkeys::events)
                .map(Message::Hotkeys)
        }
    }

    pub(super) fn hotkey_event(&mut self, event: Event) -> Task<Message> {
        let revision = match &event {
            Event::Assigned { revision, .. }
            | Event::Activated { revision, .. }
            | Event::Cancelled { revision }
            | Event::Failed { revision, .. }
            | Event::ConfigureFailed { revision, .. } => *revision,
        };
        if revision != self.hotkeys.config.revision {
            return Task::none();
        }
        match event {
            Event::Assigned { shortcuts, .. } => {
                self.hotkeys.pending = false;
                let mut changes = Vec::new();
                for assignment in shortcuts {
                    if !self
                        .hotkeys
                        .config
                        .bindings
                        .iter()
                        .any(|b| b.id == assignment.id)
                    {
                        continue;
                    }
                    if let Some(button) = self.buttons.iter_mut().find(|b| b.id == assignment.id)
                        && button.hotkey != assignment.trigger
                    {
                        button.hotkey = assignment.trigger;
                        changes.push(button.clone());
                    }
                }
                return Task::batch(
                    changes
                        .into_iter()
                        .map(|button| self.save(Change::Button(button))),
                );
            }
            Event::Activated { id, .. } => {
                if self.hotkeys.config.bindings.iter().any(|b| b.id == id)
                    && self
                        .buttons
                        .iter()
                        .any(|b| b.id == id && b.hotkey.is_some())
                {
                    self.play_button(&id);
                }
            }
            Event::Cancelled { .. } => {
                if self.hotkeys.config.configure.is_some() {
                    self.sync_hotkeys(None);
                } else {
                    self.hotkeys.pending = false;
                    self.hotkeys.error = Some(
                        "Global shortcut setup was cancelled. Click the shortcut to retry.".into(),
                    );
                }
            }
            Event::Failed { error, .. } => {
                log::warn!(target: "raspiboard::hotkeys", "{error}");
                if self.hotkeys.config.configure.is_some() {
                    self.sync_hotkeys(None);
                } else {
                    self.hotkeys.pending = false;
                }
                self.hotkeys.error = Some(error);
            }
            Event::ConfigureFailed { error, .. } => {
                log::warn!(target: "raspiboard::hotkeys", "{error}");
                self.hotkeys.pending = false;
                self.hotkeys.error = Some(error);
            }
        }
        Task::none()
    }
}
