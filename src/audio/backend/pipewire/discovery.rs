use super::graph::{Graph, Kind, Properties};
use pipewire::{
    self as pw,
    proxy::{Listener, ProxyT},
    types::ObjectType,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

struct Watched {
    _listener: Box<dyn Listener>,
    _proxy: Box<dyn ProxyT>,
}

pub struct Discovery {
    _listener: pw::registry::Listener,
    _objects: Rc<RefCell<BTreeMap<u32, Watched>>>,
    _registry: pw::registry::RegistryRc,
}

fn properties(dict: &pw::spa::utils::dict::DictRef) -> Properties {
    dict.iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

impl Discovery {
    pub fn new(core: &pw::core::CoreRc, graph: Rc<RefCell<Graph>>) -> Result<Self, String> {
        let registry = core.get_registry_rc().map_err(|e| e.to_string())?;
        let objects = Rc::new(RefCell::new(BTreeMap::new()));
        let added_objects = objects.clone();
        let removed_objects = objects.clone();
        let added_graph = graph.clone();
        let weak_registry = registry.downgrade();
        let listener = registry
            .add_listener_local()
            .global(move |object| {
                let Some(registry) = weak_registry.upgrade() else {
                    return;
                };
                let kind = match object.type_ {
                    ObjectType::Node => Kind::Node,
                    ObjectType::Port => Kind::Port,
                    ObjectType::Link => Kind::Link,
                    ObjectType::Metadata
                        if object.props.and_then(|props| props.get("metadata.name"))
                            == Some("default") =>
                    {
                        Kind::Metadata
                    }
                    _ => return,
                };
                let id = object.id;
                added_graph.borrow_mut().insert(
                    id,
                    kind,
                    object.props.map(properties).unwrap_or_default(),
                );
                let graph = added_graph.clone();
                let watched = match kind {
                    Kind::Metadata => {
                        registry
                            .bind::<pw::metadata::Metadata, _>(object)
                            .map(|proxy| {
                                let listener = proxy
                                    .add_listener_local()
                                    .property(move |subject, key, _, value| {
                                        if subject == 0 {
                                            graph.borrow_mut().default_property(key, value);
                                        }
                                        0
                                    })
                                    .register();
                                Watched {
                                    _listener: Box::new(listener),
                                    _proxy: Box::new(proxy),
                                }
                            })
                    }
                    Kind::Node => registry.bind::<pw::node::Node, _>(object).map(|proxy| {
                        let listener = proxy
                            .add_listener_local()
                            .info(move |info| {
                                if let Some(props) = info.props() {
                                    graph.borrow_mut().update(id, properties(props));
                                }
                            })
                            .register();
                        Watched {
                            _listener: Box::new(listener),
                            _proxy: Box::new(proxy),
                        }
                    }),
                    Kind::Port => registry.bind::<pw::port::Port, _>(object).map(|proxy| {
                        let listener = proxy
                            .add_listener_local()
                            .info(move |info| {
                                if let Some(props) = info.props() {
                                    graph.borrow_mut().update(id, properties(props));
                                }
                            })
                            .register();
                        Watched {
                            _listener: Box::new(listener),
                            _proxy: Box::new(proxy),
                        }
                    }),
                    Kind::Link => registry.bind::<pw::link::Link, _>(object).map(|proxy| {
                        let listener = proxy
                            .add_listener_local()
                            .info(move |info| {
                                let mut props = info.props().map(properties).unwrap_or_default();
                                let (state, error) = match info.state() {
                                    pw::link::LinkState::Active => ("active", ""),
                                    pw::link::LinkState::Paused => ("paused", ""),
                                    pw::link::LinkState::Error(error) => ("error", error),
                                    _ => ("pending", ""),
                                };
                                props.insert("link.state".into(), state.into());
                                props.insert("link.error".into(), error.into());
                                graph.borrow_mut().update(id, props);
                            })
                            .register();
                        Watched {
                            _listener: Box::new(listener),
                            _proxy: Box::new(proxy),
                        }
                    }),
                };
                if let Ok(watched) = watched {
                    added_objects.borrow_mut().insert(id, watched);
                }
            })
            .global_remove(move |id| {
                graph.borrow_mut().remove(id);
                removed_objects.borrow_mut().remove(&id);
            })
            .register();
        Ok(Self {
            _listener: listener,
            _objects: objects,
            _registry: registry,
        })
    }
}
