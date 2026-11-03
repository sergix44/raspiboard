use super::graph::{APP_ID, Graph, LOCAL_LINK_GROUP};
use crate::audio::Target;
use pipewire::{self as pw, properties::properties, proxy::ProxyT};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct OwnedLink {
    _listener: pw::proxy::ProxyListener,
    _link: pw::link::Link,
    binding: Rc<LinkBinding>,
}

#[derive(Default)]
struct LinkBinding {
    id: Cell<Option<u32>>,
    removed: Cell<bool>,
}

impl LinkBinding {
    fn ready(&self, graph: &Graph, allow_paused: bool) -> Result<bool, String> {
        if self.removed.get() {
            return Err(
                "The audio connection was removed. Enable playback again to reconnect.".into(),
            );
        }
        match self.id.get() {
            Some(id) if allow_paused => graph.link_connected(id),
            Some(id) => graph.link_ready(id),
            None => Ok(false),
        }
    }
}

pub(super) struct Sink {
    _listener: pw::proxy::ProxyListener,
    _sink: pw::node::Node,
    pub id: Rc<Cell<Option<u32>>>,
    pub name: String,
    pub error: Rc<RefCell<Option<String>>>,
}

impl Sink {
    pub fn new(
        core: &pw::core::CoreRc,
        channels: usize,
        name: String,
        description: &str,
    ) -> Result<Self, String> {
        let positions = if channels == 1 {
            "[ MONO ]"
        } else {
            "[ FL FR ]"
        };
        let sink = core.create_object::<pw::node::Node>("adapter", &properties! {
            "factory.name" => "support.null-audio-sink",
            "node.name" => name.clone(),
            "node.description" => description,
            "application.id" => APP_ID,
            "node.link-group" => LOCAL_LINK_GROUP,
            "media.class" => "Audio/Sink",
            "audio.position" => positions,
            "audio.channels" => channels.to_string(),
            "adapter.auto-port-config" => "{ mode = dsp monitor = true position = preserve }",
            "node.virtual" => "true",
            "node.autoconnect" => "false",
            "priority.session" => "0",
            "object.linger" => "false",
        }).map_err(|e| format!("Cannot create the audio mixer: {e}"))?;
        let id = Rc::new(Cell::new(None));
        let bound_id = id.clone();
        let error = Rc::new(RefCell::new(None));
        let sink_error = error.clone();
        let listener = sink
            .upcast_ref()
            .add_listener_local()
            .bound(move |id| bound_id.set(Some(id)))
            .error(move |_, _, message| *sink_error.borrow_mut() = Some(message.to_owned()))
            .register();
        Ok(Self {
            _listener: listener,
            _sink: sink,
            id,
            name,
            error,
        })
    }
}

pub struct Route {
    links: Option<Links>,
    pub sink: Sink,
    target: u32,
}

impl Route {
    pub fn new(
        core: &pw::core::CoreRc,
        graph: &Graph,
        target: Target,
        sequence: u64,
    ) -> Result<Self, String> {
        let target = graph.resolve(&target)?;
        let channels = graph.channels(target, "in")?.len();
        let sink = Sink::new(
            core,
            channels,
            format!("raspiboard.{}.{}", std::process::id(), sequence),
            "Raspiboard clips",
        )?;
        Ok(Self {
            links: None,
            sink,
            target,
        })
    }

    pub fn prepare(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<bool, String> {
        if let Some(error) = self.sink.error.borrow_mut().take() {
            return Err(error);
        }
        let Some(sink) = self.sink.id.get() else {
            return Ok(false);
        };
        self.links
            .get_or_insert_with(|| Links::new(sink, self.target))
            .prepare(core, graph)
    }
}

pub(super) struct Links {
    links: Vec<OwnedLink>,
    error: Rc<RefCell<Option<String>>>,
    source: u32,
    target: u32,
    pairs: Vec<(u32, u32)>,
    remix: bool,
}

impl Links {
    pub fn new(source: u32, target: u32) -> Self {
        Self {
            links: Vec::new(),
            error: Rc::default(),
            source,
            target,
            pairs: Vec::new(),
            remix: false,
        }
    }

    pub fn mixing(source: u32, target: u32) -> Self {
        Self {
            remix: true,
            ..Self::new(source, target)
        }
    }

    fn port_pairs(&self, graph: &Graph) -> Result<Vec<(u32, u32)>, String> {
        if self.remix {
            graph.mix_port_pairs(self.source, self.target)
        } else {
            graph.port_pairs(self.source, self.target)
        }
    }

    pub fn prepare(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<bool, String> {
        self.prepare_with(core, graph, false)
    }

    pub fn prepare_capture(
        &mut self,
        core: &pw::core::CoreRc,
        graph: &Graph,
    ) -> Result<bool, String> {
        self.prepare_with(core, graph, true)
    }

    fn prepare_with(
        &mut self,
        core: &pw::core::CoreRc,
        graph: &Graph,
        allow_paused: bool,
    ) -> Result<bool, String> {
        if let Some(error) = self.error.borrow_mut().take() {
            return Err(error);
        }
        if self.links.is_empty() {
            let Ok(pairs) = self.port_pairs(graph) else {
                return Ok(false);
            };
            for &(output, input) in &pairs {
                let link = core
                    .create_object::<pw::link::Link>(
                        "link-factory",
                        &properties! {
                            "link.output.node" => self.source.to_string(),
                            "link.output.port" => output.to_string(),
                            "link.input.node" => self.target.to_string(),
                            "link.input.port" => input.to_string(),
                            "object.linger" => "false",
                        },
                    )
                    .map_err(|e| format!("Cannot connect the selected application: {e}"))?;
                let binding = Rc::new(LinkBinding::default());
                let bound = binding.clone();
                let removed = binding.clone();
                let error = self.error.clone();
                let listener = link
                    .upcast_ref()
                    .add_listener_local()
                    .bound(move |id| bound.id.set(Some(id)))
                    .removed(move || removed.removed.set(true))
                    .error(move |_, _, message| *error.borrow_mut() = Some(message.to_owned()))
                    .register();
                self.links.push(OwnedLink {
                    _listener: listener,
                    _link: link,
                    binding,
                });
            }
            self.pairs = pairs;
        }
        if self.port_pairs(graph).ok().as_ref() != Some(&self.pairs) {
            return Err("The application's audio channels changed. Select it again.".into());
        }
        self.links.iter().try_fold(true, |ready, link| {
            let active = link.binding.ready(graph, allow_paused)?;
            Ok(ready && active)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::graph::Kind;
    use super::*;

    #[test]
    fn passthrough_removed_owned_link_cannot_be_revived_by_a_reused_global_id() {
        let binding = LinkBinding::default();
        binding.id.set(Some(99));
        let mut graph = Graph::default();
        graph.insert(
            99,
            Kind::Link,
            [("link.state".into(), "active".into())].into(),
        );
        assert!(binding.ready(&graph, true).unwrap());
        binding.removed.set(true);
        graph.remove(99);
        graph.insert(
            99,
            Kind::Link,
            [("link.state".into(), "paused".into())].into(),
        );
        assert!(binding.ready(&graph, true).unwrap_err().contains("removed"));
    }
}
