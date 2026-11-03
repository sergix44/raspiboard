use std::collections::{BTreeMap, BTreeSet};

pub use crate::APP_ID;
use crate::audio::{Target, TargetId};
pub const LOCAL_LINK_GROUP: &str = "io.raspiboard.Raspiboard.local";

pub type Properties = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Node,
    Port,
    Link,
    Metadata,
}

#[derive(Default)]
pub struct Graph {
    objects: BTreeMap<u32, (Kind, Properties)>,
    defaults: BTreeMap<String, String>,
}

impl Graph {
    pub fn resolve(&self, target: &Target) -> Result<u32, String> {
        self.objects
            .iter()
            .find_map(|(&id, (kind, props))| {
                if *kind != Kind::Node {
                    return None;
                }
                let serial = props.get("object.serial")?.parse().ok()?;
                (identity(id, serial) == target.id).then_some(id)
            })
            .ok_or_else(|| "The selected audio stream closed. Select an application again.".into())
    }

    pub fn insert(&mut self, id: u32, kind: Kind, props: Properties) {
        self.objects.insert(id, (kind, props));
    }

    pub fn remove(&mut self, id: u32) {
        if self
            .objects
            .remove(&id)
            .is_some_and(|(kind, _)| kind == Kind::Metadata)
        {
            self.defaults.clear();
        }
    }

    pub fn default_property(&mut self, key: Option<&str>, value: Option<&str>) {
        let Some(key) = key else {
            self.defaults.clear();
            return;
        };
        if !matches!(key, "default.audio.sink" | "default.audio.source") {
            return;
        }
        let name = value
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .and_then(|value| {
                value
                    .get("name")
                    .and_then(|name| name.as_str())
                    .map(str::to_owned)
            });
        if let Some(name) = name {
            self.defaults.insert(key.into(), name);
        } else {
            self.defaults.remove(key);
        }
    }

    pub fn recording_devices(&self) -> Option<(Target, Target)> {
        let endpoint = |key, class| {
            let name = self.defaults.get(key)?;
            self.objects.iter().find_map(|(&id, (kind, props))| {
                if *kind != Kind::Node
                    || value(props, "node.name") != name
                    || value(props, "media.class") != class
                    || value(props, "application.id") == APP_ID
                {
                    return None;
                }
                Some(Target {
                    id: identity(id, props.get("object.serial")?.parse().ok()?),
                    display_label: name.clone(),
                    label: name.clone(),
                })
            })
        };
        Some((
            endpoint("default.audio.sink", "Audio/Sink")?,
            endpoint("default.audio.source", "Audio/Source")?,
        ))
    }

    pub fn feeds(&self, source: u32, target: u32) -> bool {
        let mut pending = vec![target];
        let mut visited = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if node == source {
                return true;
            }
            if !visited.insert(node) {
                continue;
            }
            pending.extend(self.objects.values().filter_map(|(kind, props)| {
                (*kind == Kind::Link && number(props, "link.input.node") == Some(node))
                    .then(|| number(props, "link.output.node"))
                    .flatten()
            }));
        }
        false
    }

    pub fn update(&mut self, id: u32, props: Properties) {
        if let Some((_, current)) = self.objects.get_mut(&id) {
            current.extend(props);
        }
    }

    pub fn targets(&self) -> Vec<Target> {
        self.objects
            .iter()
            .filter_map(|(&id, (kind, props))| {
                if *kind != Kind::Node
                    || value(props, "media.class") != "Stream/Input/Audio"
                    || value(props, "application.id") == APP_ID
                    || value(props, "media.role") == "DSP"
                {
                    return None;
                }
                let (microphone, monitor) = self.source_kind(id);
                if monitor && !microphone {
                    return None;
                }
                let source = if microphone {
                    "microphone"
                } else {
                    "source unknown"
                };
                let name = ["application.name", "node.description", "node.name"]
                    .iter()
                    .find_map(|key| props.get(*key))
                    .map(String::as_str)
                    .unwrap_or("Unnamed application");
                let description = props
                    .get("media.name")
                    .map(|s| format!(" — {s}"))
                    .unwrap_or_default();
                Some(Target {
                    id: identity(id, props.get("object.serial")?.parse().ok()?),
                    display_label: compact_label(name, id),
                    label: format!("{name}{description} [{id}] — {source}"),
                })
            })
            .collect()
    }

    pub fn sources(&self) -> Vec<Target> {
        self.objects
            .iter()
            .filter_map(|(&id, (kind, props))| {
                if *kind != Kind::Node
                    || value(props, "media.class") != "Stream/Output/Audio"
                    || value(props, "application.id") == APP_ID
                    || value(props, "media.role") == "DSP"
                {
                    return None;
                }
                let name = ["application.name", "node.description", "node.name"]
                    .iter()
                    .find_map(|key| props.get(*key))
                    .map(String::as_str)
                    .unwrap_or("Unnamed application");
                Some(Target {
                    id: identity(id, props.get("object.serial")?.parse().ok()?),
                    display_label: compact_label(name, id),
                    label: format!("{name} [{}] — {}", id, value(props, "media.name")),
                })
            })
            .collect()
    }

    fn source_kind(&self, target: u32) -> (bool, bool) {
        let mut visited = BTreeSet::new();
        let mut pending = vec![target];
        let (mut microphone, mut monitor) = (false, false);
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            for (_, props) in self.objects.values().filter(|(kind, props)| {
                *kind == Kind::Link && number(props, "link.input.node") == Some(id)
            }) {
                let Some(source) = number(props, "link.output.node") else {
                    continue;
                };
                let Some((_, node)) = self.objects.get(&source) else {
                    continue;
                };
                if value(node, "application.id") == APP_ID {
                    continue;
                }
                let is_monitor = number(props, "link.output.port")
                    .and_then(|port| self.objects.get(&port))
                    .is_some_and(|(_, port)| value(port, "port.monitor") == "true");
                let class = value(node, "media.class");
                if class == "Audio/Sink" || (is_monitor && !class.starts_with("Audio/Source")) {
                    monitor = true;
                    continue;
                }
                if value(node, "media.class") == "Audio/Source" && node.contains_key("device.id") {
                    microphone = true;
                }
                pending.push(source);
            }
        }
        (microphone, monitor)
    }

    pub fn channels(&self, node: u32, direction: &str) -> Result<BTreeMap<String, u32>, String> {
        let mut channels = BTreeMap::new();
        for (&id, (_, props)) in self.objects.iter().filter(|(_, (kind, props))| {
            *kind == Kind::Port
                && number(props, "node.id") == Some(node)
                && value(props, "port.direction") == direction
        }) {
            let channel = value(props, "audio.channel");
            if channel.is_empty() || channels.insert(channel.to_owned(), id).is_some() {
                return Err("The application has an unknown or ambiguous channel layout.".into());
            }
        }
        let names: Vec<_> = channels.keys().map(String::as_str).collect();
        if names != ["MONO"] && names != ["FL", "FR"] {
            return Err("The application must expose mono or stereo audio inputs.".into());
        }
        Ok(channels)
    }

    pub fn port_pairs(&self, source: u32, target: u32) -> Result<Vec<(u32, u32)>, String> {
        let outputs = self.channels(source, "out")?;
        self.channels(target, "in")?
            .into_iter()
            .map(|(channel, input)| {
                outputs
                    .get(&channel)
                    .copied()
                    .map(|output| (output, input))
                    .ok_or_else(|| format!("No output is available for channel {channel}."))
            })
            .collect()
    }

    pub fn mix_port_pairs(&self, source: u32, target: u32) -> Result<Vec<(u32, u32)>, String> {
        let outputs = self.channels(source, "out")?;
        if let Some(&mono) = outputs.get("MONO") {
            Ok(self
                .channels(target, "in")?
                .values()
                .map(|&input| (mono, input))
                .collect())
        } else {
            self.port_pairs(source, target)
        }
    }

    pub fn has_link(&self, source: u32, target: u32) -> Result<bool, String> {
        let Ok(channels) = self.channels(target, "in") else {
            return Ok(false);
        };
        let mut ready_ports = BTreeSet::new();
        for (&id, (kind, props)) in &self.objects {
            if *kind == Kind::Link
                && number(props, "link.output.node") == Some(source)
                && number(props, "link.input.node") == Some(target)
                && self.link_ready(id)?
                && let Some(port) = number(props, "link.input.port")
            {
                ready_ports.insert(port);
            }
        }
        Ok(channels.values().all(|port| ready_ports.contains(port)))
    }

    pub fn link_ready(&self, id: u32) -> Result<bool, String> {
        let Some((Kind::Link, props)) = self.objects.get(&id) else {
            return Ok(false);
        };
        match value(props, "link.state") {
            "error" => Err(format!(
                "Audio connection failed: {}",
                value(props, "link.error")
            )),
            "active" => Ok(true),
            _ => Ok(false),
        }
    }

    pub fn link_connected(&self, id: u32) -> Result<bool, String> {
        if self.objects.get(&id).is_some_and(|(kind, props)| {
            *kind == Kind::Link && value(props, "link.state") == "paused"
        }) {
            Ok(true)
        } else {
            self.link_ready(id)
        }
    }
}

fn value<'a>(props: &'a Properties, key: &str) -> &'a str {
    props.get(key).map(String::as_str).unwrap_or_default()
}

fn number(props: &Properties, key: &str) -> Option<u32> {
    props.get(key)?.parse().ok()
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

fn identity(id: u32, serial: u64) -> TargetId {
    format!("pipewire:{id}:{serial}").into()
}
fn compact_label(name: &str, id: u32) -> String {
    let name = name.split(" — ").next().unwrap_or(name);
    let suffix = format!("[{id}]");
    if name.ends_with(&suffix) {
        name.into()
    } else {
        format!("{name} {suffix}")
    }
}
