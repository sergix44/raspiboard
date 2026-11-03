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
    id: Rc<Cell<Option<u32>>>,
}

pub struct Route {
    links: Vec<OwnedLink>,
    _listener: pw::proxy::ProxyListener,
    _sink: pw::node::Node,
    pub id: Rc<Cell<Option<u32>>>,
    pub name: String,
    pub error: Rc<RefCell<Option<String>>>,
    target: u32,
    pairs: Vec<(u32, u32)>,
}

impl Route {
    pub fn new(
        core: &pw::core::CoreRc,
        graph: &Graph,
        target: Target,
        sequence: u64,
    ) -> Result<Self, String> {
        let target = graph.resolve(&target)?;
        let channels = graph.channels(target, "in")?;
        let positions = if channels.len() == 1 {
            "[ MONO ]"
        } else {
            "[ FL FR ]"
        };
        let name = format!("raspiboard.{}.{}", std::process::id(), sequence);
        let sink = core.create_object::<pw::node::Node>("adapter", &properties! {
            "factory.name" => "support.null-audio-sink",
            "node.name" => name.clone(),
            "node.description" => "Raspiboard clips",
            "application.id" => APP_ID,
            "node.link-group" => LOCAL_LINK_GROUP,
            "media.class" => "Audio/Sink",
            "audio.position" => positions,
            "audio.channels" => channels.len().to_string(),
            "adapter.auto-port-config" => "{ mode = dsp monitor = true position = preserve }",
            "node.virtual" => "true",
            "node.autoconnect" => "false",
            "priority.session" => "0",
            "object.linger" => "false",
        }).map_err(|e| format!("Cannot create the clip output: {e}"))?;
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
            links: Vec::new(),
            _listener: listener,
            _sink: sink,
            id,
            name,
            error,
            target,
            pairs: Vec::new(),
        })
    }

    pub fn prepare(&mut self, core: &pw::core::CoreRc, graph: &Graph) -> Result<bool, String> {
        if let Some(error) = self.error.borrow_mut().take() {
            return Err(error);
        }
        let Some(sink) = self.id.get() else {
            return Ok(false);
        };
        if self.links.is_empty() {
            let Ok(pairs) = graph.port_pairs(sink, self.target) else {
                return Ok(false);
            };
            for &(output, input) in &pairs {
                let link = core
                    .create_object::<pw::link::Link>(
                        "link-factory",
                        &properties! {
                            "link.output.node" => sink.to_string(),
                            "link.output.port" => output.to_string(),
                            "link.input.node" => self.target.to_string(),
                            "link.input.port" => input.to_string(),
                            "object.linger" => "false",
                        },
                    )
                    .map_err(|e| format!("Cannot connect the selected application: {e}"))?;
                let id = Rc::new(Cell::new(None));
                let bound_id = id.clone();
                let error = self.error.clone();
                let listener = link
                    .upcast_ref()
                    .add_listener_local()
                    .bound(move |id| bound_id.set(Some(id)))
                    .error(move |_, _, message| *error.borrow_mut() = Some(message.to_owned()))
                    .register();
                self.links.push(OwnedLink {
                    _listener: listener,
                    _link: link,
                    id,
                });
            }
            self.pairs = pairs;
        }
        if graph.port_pairs(sink, self.target).ok().as_ref() != Some(&self.pairs) {
            return Err("The application's audio channels changed. Select it again.".into());
        }
        self.links.iter().try_fold(true, |ready, link| {
            let active = match link.id.get() {
                Some(id) => graph.link_ready(id)?,
                None => false,
            };
            Ok(ready && active)
        })
    }
}
