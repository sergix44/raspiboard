use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
use iced::futures::{SinkExt, Stream, StreamExt, channel::mpsc, stream};
use std::collections::HashSet;

use super::{Assignment, Config, Event};
pub fn events(config: &Config) -> impl Stream<Item = Event> + use<> {
    let config = config.clone();
    iced::stream::channel(8, async move |mut output: mpsc::Sender<Event>| {
        if let Err(error) = run(&config, &mut output).await {
            let event = if matches!(
                error,
                ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)
            ) {
                Event::Cancelled {
                    revision: config.revision,
                }
            } else {
                Event::Failed {
                    revision: config.revision,
                    error: format!(
                        "Global shortcuts are unavailable: {error}. Click Set or the shortcut to retry."
                    ),
                }
            };
            let _ = output.send(event).await;
        }
    })
}

async fn run(config: &Config, output: &mut mpsc::Sender<Event>) -> ashpd::Result<()> {
    // This connection belongs to the subscription: dropping it also closes any
    // session or chooser still being created when the configuration changes.
    let connection = zbus::Connection::session().await?;
    ashpd::register_host_app_with_connection(connection.clone(), crate::APP_ID.try_into()?).await?;
    let portal = GlobalShortcuts::with_connection(connection).await?;
    let session = portal.create_session(Default::default()).await?;
    // ashpd exposes the session's object path through Serialize only.
    let path = serde_json::to_value(&session)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(ashpd::Error::ParseError(
            "Invalid global shortcut session path",
        ))?;
    let mut active =
        ActivationState::new(path.clone(), config.bindings.iter().map(|b| b.id.clone()));
    let shortcuts = portal_signals(portal.receive_all_signals().await?);
    let closed = session.receive_closed().await?.map(|_| Ok(Signal::Closed));
    let signals = stream::select(shortcuts, closed);
    iced::futures::pin_mut!(signals);

    let previous = portal
        .list_shortcuts(&session, Default::default())
        .await?
        .response()?;
    let returning = config
        .configure
        .as_ref()
        .is_some_and(|id| previous.shortcuts().iter().any(|s| s.id() == id));
    let shortcuts: Vec<_> = config
        .bindings
        .iter()
        .map(|binding| NewShortcut::new(&binding.id, &binding.description))
        .collect();
    let response = portal
        .bind_shortcuts(&session, &shortcuts, None, Default::default())
        .await?
        .response()?;
    if output
        .send(Event::Assigned {
            revision: config.revision,
            shortcuts: bound_assignments(config, response.shortcuts()),
        })
        .await
        .is_err()
    {
        return Ok(());
    }
    // New actions are configured by BindShortcuts; returning actions use the
    // desktop's configuration UI instead of opening two choosers in succession.
    if returning
        && let Err(error) = portal
            .configure_shortcuts(&session, None, Default::default())
            .await
    {
        let _ = output.send(Event::ConfigureFailed {
            revision: config.revision,
            error: format!("Cannot open the shortcut chooser: {error}. Edit the app shortcuts in your desktop settings."),
        }).await;
    }
    while let Some(signal) = signals.next().await {
        let event = match signal? {
            Signal::Pressed(event)
                if active.press(event.session_handle().as_str(), event.shortcut_id()) =>
            {
                Some(Event::Activated {
                    revision: config.revision,
                    id: event.shortcut_id().into(),
                })
            }
            Signal::Released(event) => {
                active.release(event.session_handle().as_str(), event.shortcut_id());
                None
            }
            Signal::Changed(event) if event.session_handle().as_str() == path => {
                Some(Event::Assigned {
                    revision: config.revision,
                    shortcuts: assignments(event.shortcuts()),
                })
            }
            Signal::Closed => return Err(ashpd::Error::NoResponse),
            _ => None,
        };
        if let Some(event) = event
            && output.send(event).await.is_err()
        {
            return Ok(());
        }
    }
    Err(ashpd::Error::NoResponse)
}

enum Signal {
    Pressed(ashpd::desktop::global_shortcuts::Activated),
    Released(ashpd::desktop::global_shortcuts::Deactivated),
    Changed(ashpd::desktop::global_shortcuts::ShortcutsChanged),
    Closed,
    Ignored,
}

fn portal_signals(
    messages: impl Stream<Item = zbus::Message>,
) -> impl Stream<Item = ashpd::Result<Signal>> {
    // Keep a single queue: merging separate press/release streams can reorder
    // overlapping shortcuts and leave a released key marked as held.
    messages.map(
        |message| match message.header().member().map(|member| member.as_str()) {
            Some("Activated") => Ok(Signal::Pressed(message.body().deserialize()?)),
            Some("Deactivated") => Ok(Signal::Released(message.body().deserialize()?)),
            Some("ShortcutsChanged") => Ok(Signal::Changed(message.body().deserialize()?)),
            _ => Ok(Signal::Ignored),
        },
    )
}

fn bound_assignments(config: &Config, shortcuts: &[Shortcut]) -> Vec<Assignment> {
    let mut bound = assignments(shortcuts);
    for binding in &config.bindings {
        if !bound.iter().any(|assignment| assignment.id == binding.id) {
            bound.push(Assignment {
                id: binding.id.clone(),
                trigger: None,
            });
        }
    }
    bound
}

fn assignments(shortcuts: &[Shortcut]) -> Vec<Assignment> {
    shortcuts
        .iter()
        .map(|shortcut| Assignment {
            id: shortcut.id().into(),
            trigger: (!shortcut.trigger_description().trim().is_empty())
                .then(|| shortcut.trigger_description().to_owned()),
        })
        .collect()
}

struct ActivationState {
    session: String,
    members: HashSet<String>,
    pressed: HashSet<String>,
}

impl ActivationState {
    fn new(session: String, members: impl IntoIterator<Item = String>) -> Self {
        Self {
            session,
            members: members.into_iter().collect(),
            pressed: HashSet::new(),
        }
    }

    fn press(&mut self, session: &str, id: &str) -> bool {
        self.session == session && self.members.contains(id) && self.pressed.insert(id.into())
    }

    fn release(&mut self, session: &str, id: &str) {
        if self.session == session {
            self.pressed.remove(id);
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
use super::Binding;
