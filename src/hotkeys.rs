use iced::futures::Stream;
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Binding {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Hash)]
pub struct Config {
    pub revision: u64,
    pub bindings: Vec<Binding>,
    pub configure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub id: String,
    pub trigger: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Event {
    Assigned {
        revision: u64,
        shortcuts: Vec<Assignment>,
    },
    Activated {
        revision: u64,
        id: String,
    },
    Cancelled {
        revision: u64,
    },
    Failed {
        revision: u64,
        error: String,
    },
    ConfigureFailed {
        revision: u64,
        error: String,
    },
}

#[cfg(target_os = "linux")]
mod portal;
pub fn events(config: &Config) -> impl Stream<Item = Event> + use<> {
    portal::events(config)
}
