use super::*;

#[test]
fn recording_uses_default_devices_and_duplicates_a_mono_microphone_into_stereo() {
    let mut graph = Graph::default();
    graph.insert(
        10,
        Kind::Node,
        props(&[
            ("media.class", "Audio/Sink"),
            ("node.name", "speakers"),
            ("object.serial", "100"),
        ]),
    );
    graph.insert(
        20,
        Kind::Node,
        props(&[
            ("media.class", "Audio/Source"),
            ("node.name", "microphone"),
            ("object.serial", "200"),
        ]),
    );
    graph.default_property(Some("default.audio.sink"), Some(r#"{"name":"speakers"}"#));
    graph.default_property(
        Some("default.audio.source"),
        Some(r#"{"name":"microphone"}"#),
    );
    let (output, microphone) = graph.recording_devices().unwrap();
    assert_eq!(graph.resolve(&output).unwrap(), 10);
    assert_eq!(graph.resolve(&microphone).unwrap(), 20);
    graph.insert(
        21,
        Kind::Port,
        props(&[
            ("node.id", "20"),
            ("port.direction", "out"),
            ("audio.channel", "MONO"),
        ]),
    );
    for (id, channel) in [(31, "FL"), (32, "FR")] {
        graph.insert(
            id,
            Kind::Port,
            props(&[
                ("node.id", "30"),
                ("port.direction", "in"),
                ("audio.channel", channel),
            ]),
        );
    }
    assert_eq!(
        graph.mix_port_pairs(20, 30).unwrap(),
        vec![(21, 31), (21, 32)]
    );
    graph.default_property(Some("default.audio.source"), None);
    assert!(graph.recording_devices().is_none());
}

#[test]
fn recording_does_not_add_the_microphone_twice_when_it_is_already_monitored() {
    let mut graph = Graph::default();
    graph.insert(
        90,
        Kind::Link,
        props(&[("link.output.node", "20"), ("link.input.node", "15")]),
    );
    graph.insert(
        91,
        Kind::Link,
        props(&[("link.output.node", "15"), ("link.input.node", "10")]),
    );
    assert!(graph.feeds(20, 10));
    assert!(!graph.feeds(30, 10));
}

#[test]
fn passthrough_capture_accepts_paused_links_but_not_negotiation_or_errors() {
    let mut graph = Graph::default();
    graph.insert(99, Kind::Link, props(&[("link.state", "paused")]));
    assert!(graph.link_connected(99).unwrap());
    assert!(!graph.link_ready(99).unwrap());
    graph.update(99, props(&[("link.state", "negotiating")]));
    assert!(!graph.link_connected(99).unwrap());
    graph.update(
        99,
        props(&[("link.state", "error"), ("link.error", "denied")]),
    );
    assert!(graph.link_connected(99).unwrap_err().contains("denied"));
    graph.remove(99);
    assert!(!graph.link_connected(99).unwrap());
}

#[test]
fn passthrough_discovers_only_external_playback_streams_and_checks_serials() {
    let mut graph = Graph::default();
    for (id, class, app, role) in [
        (10, "Stream/Output/Audio", "browser", "Music"),
        (20, "Stream/Input/Audio", "recorder", "Communication"),
        (30, "Stream/Output/Audio", APP_ID, "Music"),
        (40, "Audio/Sink", "desktop", "Music"),
        (50, "Stream/Output/Audio", "filter", "DSP"),
    ] {
        graph.insert(
            id,
            Kind::Node,
            props(&[
                ("object.serial", &id.to_string()),
                ("media.class", class),
                ("application.id", app),
                ("application.name", app),
                ("media.role", role),
            ]),
        );
    }
    let sources = graph.sources();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].display_label, "browser [10]");
    assert_eq!(graph.resolve(&sources[0]).unwrap(), 10);
    graph.update(10, props(&[("object.serial", "999")]));
    assert!(graph.resolve(&sources[0]).is_err());
    assert_ne!(graph.sources()[0].id, sources[0].id);
}

#[test]
fn existing_but_failed_or_negotiating_links_are_not_ready() {
    let mut graph = Graph::default();
    graph.insert(
        21,
        Kind::Port,
        props(&[
            ("node.id", "20"),
            ("port.direction", "in"),
            ("audio.channel", "MONO"),
        ]),
    );
    graph.insert(
        30,
        Kind::Link,
        props(&[
            ("link.output.node", "10"),
            ("link.input.node", "20"),
            ("link.input.port", "21"),
            ("link.state", "negotiating"),
        ]),
    );
    assert!(!graph.has_link(10, 20).unwrap());
    graph.update(
        30,
        props(&[("link.state", "error"), ("link.error", "format mismatch")]),
    );
    assert!(
        graph
            .has_link(10, 20)
            .unwrap_err()
            .contains("format mismatch")
    );
    graph.update(30, props(&[("link.state", "active")]));
    assert!(graph.has_link(10, 20).unwrap());
}

#[test]
fn waits_for_every_playback_channel_before_starting() {
    let mut graph = Graph::default();
    for (id, channel) in [(21, "FL"), (22, "FR")] {
        graph.insert(
            id,
            Kind::Port,
            props(&[
                ("node.id", "20"),
                ("port.direction", "in"),
                ("audio.channel", channel),
            ]),
        );
    }
    graph.insert(
        30,
        Kind::Link,
        props(&[
            ("link.output.node", "10"),
            ("link.input.node", "20"),
            ("link.input.port", "21"),
            ("link.state", "active"),
        ]),
    );
    assert!(!graph.has_link(10, 20).unwrap());
    graph.insert(
        31,
        Kind::Link,
        props(&[
            ("link.output.node", "10"),
            ("link.input.node", "20"),
            ("link.input.port", "22"),
            ("link.state", "active"),
        ]),
    );
    assert!(graph.has_link(10, 20).unwrap());
}

#[test]
fn discovers_capture_streams_through_a_recorded_filter_graph() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/filtered-input.json"
    ))
    .unwrap();
    let mut graph = Graph::default();
    for object in fixture.as_array().unwrap() {
        let kind = match object["kind"].as_str().unwrap() {
            "Node" => Kind::Node,
            "Port" => Kind::Port,
            "Link" => Kind::Link,
            _ => unreachable!(),
        };
        let props = serde_json::from_value(object["props"].clone()).unwrap();
        graph.insert(object["id"].as_u64().unwrap() as u32, kind, props);
    }
    let targets = graph.targets();
    assert_eq!(
        targets
            .iter()
            .map(|target| graph.resolve(target).unwrap())
            .collect::<Vec<_>>(),
        vec![178, 244, 251]
    );
    assert!(
        targets
            .iter()
            .all(|target| target.label.contains("microphone"))
    );
}

fn props(values: &[(&str, &str)]) -> Properties {
    values
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn capture(graph: &mut Graph, id: u32, serial: &str) {
    graph.insert(
        id,
        Kind::Node,
        props(&[
            ("media.class", "Stream/Input/Audio"),
            ("object.serial", serial),
            ("application.name", "An unfamiliar application"),
        ]),
    );
}

fn link(graph: &mut Graph, id: u32, source: &str, target: &str, port: &str) {
    graph.insert(
        id,
        Kind::Link,
        props(&[
            ("link.output.node", source),
            ("link.input.node", target),
            ("link.output.port", port),
        ]),
    );
}

#[test]
fn discovers_unknown_applications_connected_to_a_microphone() {
    let mut graph = Graph::default();
    capture(&mut graph, 20, "200");
    graph.insert(
        10,
        Kind::Node,
        props(&[("media.class", "Audio/Source"), ("device.id", "1")]),
    );
    link(&mut graph, 30, "10", "20", "11");
    let targets = graph.targets();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].id, identity(20, 200));
    assert!(targets[0].label.contains("An unfamiliar application"));
    assert!(targets[0].label.contains("microphone"));
}

#[test]
fn excludes_desktop_monitors_and_own_nodes() {
    let mut graph = Graph::default();
    capture(&mut graph, 20, "200");
    graph.insert(10, Kind::Node, props(&[("media.class", "Audio/Sink")]));
    link(&mut graph, 30, "10", "20", "11");
    graph.insert(
        40,
        Kind::Node,
        props(&[
            ("media.class", "Stream/Input/Audio"),
            ("object.serial", "400"),
            ("application.id", "io.raspiboard.Raspiboard"),
        ]),
    );
    assert!(graph.targets().is_empty());
}

#[test]
fn follows_filter_chains_and_handles_cycles() {
    let mut graph = Graph::default();
    capture(&mut graph, 20, "200");
    graph.insert(
        10,
        Kind::Node,
        props(&[("media.class", "Audio/Source"), ("device.id", "1")]),
    );
    graph.insert(
        12,
        Kind::Node,
        props(&[("media.class", "Audio/Source/Virtual")]),
    );
    link(&mut graph, 30, "10", "12", "11");
    link(&mut graph, 31, "12", "20", "13");
    link(&mut graph, 32, "20", "12", "21");
    assert!(graph.targets()[0].label.contains("microphone"));
}

#[test]
fn keeps_unresolved_sources_visible_and_distinguishes_reused_ids() {
    let mut graph = Graph::default();
    capture(&mut graph, 20, "200");
    let first = graph.targets().pop().unwrap();
    assert!(first.label.contains("source unknown"));
    graph.remove(20);
    assert!(graph.targets().is_empty());
    capture(&mut graph, 20, "999");
    assert_ne!(first.id, graph.targets()[0].id);
}

#[test]
fn matches_channels_instead_of_port_order_or_names() {
    let mut graph = Graph::default();
    for (id, node, direction, channel) in [
        (11, "10", "out", "FR"),
        (12, "10", "out", "FL"),
        (21, "20", "in", "FL"),
        (22, "20", "in", "FR"),
    ] {
        graph.insert(
            id,
            Kind::Port,
            props(&[
                ("node.id", node),
                ("port.direction", direction),
                ("audio.channel", channel),
            ]),
        );
    }
    assert_eq!(graph.port_pairs(10, 20).unwrap(), vec![(12, 21), (11, 22)]);
}

#[test]
fn rejects_incomplete_or_unknown_channel_layouts() {
    let mut graph = Graph::default();
    graph.insert(
        21,
        Kind::Port,
        props(&[("node.id", "20"), ("port.direction", "in")]),
    );
    assert!(graph.port_pairs(10, 20).is_err());
}

#[test]
fn virtual_microphone_monitor_ports_are_not_desktop_recording() {
    let mut graph = Graph::default();
    capture(&mut graph, 20, "200");
    graph.insert(
        10,
        Kind::Node,
        props(&[("media.class", "Audio/Source"), ("device.id", "1")]),
    );
    graph.insert(
        12,
        Kind::Node,
        props(&[("media.class", "Audio/Source/Virtual")]),
    );
    graph.insert(
        13,
        Kind::Port,
        props(&[("node.id", "12"), ("port.monitor", "true")]),
    );
    link(&mut graph, 30, "10", "12", "11");
    link(&mut graph, 31, "12", "20", "13");
    let targets = graph.targets();
    assert_eq!(targets.len(), 1);
    assert!(targets[0].label.contains("microphone"));
}

#[test]
fn target_labels_do_not_change_identity_and_stale_ids_cannot_resolve() {
    let mut graph = Graph::default();
    capture(&mut graph, 20, "200");
    let original = graph.targets().remove(0);
    graph.update(20, props(&[("application.name", "Renamed")]));
    let renamed = graph.targets().remove(0);
    assert_eq!(original.id, renamed.id);
    assert_ne!(original.display_label, renamed.display_label);
    assert_eq!(graph.resolve(&original), Ok(20));
    graph.update(20, props(&[("object.serial", "201")]));
    assert!(graph.resolve(&original).is_err());
}

#[test]
fn compact_labels_preserve_existing_application_presentation() {
    for name in ["Recorder", "Recorder [20]", "Recorder — Capture audio"] {
        assert_eq!(compact_label(name, 20), "Recorder [20]");
    }
}
