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
fn removing_a_node_discards_its_ports_before_the_node_id_can_be_reused() {
    let mut graph = Graph::default();
    graph.insert(10, Kind::Node, props(&[("object.serial", "100")]));
    graph.insert(20, Kind::Node, props(&[("object.serial", "200")]));
    for (id, node, direction) in [(11, "10", "out"), (21, "20", "in")] {
        graph.insert(
            id,
            Kind::Port,
            props(&[
                ("node.id", node),
                ("port.direction", direction),
                ("audio.channel", "MONO"),
            ]),
        );
    }
    graph.insert(
        30,
        Kind::Link,
        props(&[
            ("link.output.node", "10"),
            ("link.output.port", "11"),
            ("link.input.node", "20"),
            ("link.input.port", "21"),
            ("link.state", "active"),
        ]),
    );
    assert_eq!(graph.port_pairs(10, 20).unwrap(), vec![(11, 21)]);

    graph.remove(10);
    graph.insert(10, Kind::Node, props(&[("object.serial", "999")]));
    assert!(
        graph.port_pairs(10, 20).is_err(),
        "old ports must not be offered for a new sink with a recycled node ID"
    );
    assert!(!graph.link_ready(30).unwrap());
    assert!(!graph.feeds(10, 20));
    graph.insert(
        12,
        Kind::Port,
        props(&[
            ("node.id", "10"),
            ("port.direction", "out"),
            ("audio.channel", "MONO"),
        ]),
    );
    assert_eq!(graph.port_pairs(10, 20).unwrap(), vec![(12, 21)]);
}

#[test]
fn removing_a_port_discards_connections_that_still_reference_it() {
    let mut graph = Graph::default();
    graph.insert(
        11,
        Kind::Port,
        props(&[
            ("node.id", "10"),
            ("port.direction", "out"),
            ("audio.channel", "MONO"),
        ]),
    );
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
            ("link.output.port", "11"),
            ("link.input.node", "20"),
            ("link.input.port", "21"),
            ("link.state", "active"),
        ]),
    );
    graph.remove(11);
    assert!(!graph.link_ready(30).unwrap());
    assert!(!graph.has_link(10, 20).unwrap());
    assert!(!graph.feeds(10, 20));
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
        assert_eq!(compact_label(name, 20, None), "Recorder [20]");
    }
}

type Discover = fn(&Graph) -> Vec<Target>;

const SELECTORS: [(&str, Discover); 2] = [
    ("Stream/Input/Audio", Graph::targets),
    ("Stream/Output/Audio", Graph::sources),
];

fn named_stream(graph: &mut Graph, id: u32, class: &str, metadata: &[(&str, &str)]) {
    capture(graph, id, &format!("{id}00"));
    graph.update(
        id,
        props(&[("application.name", "Browser"), ("media.class", class)]),
    );
    graph.update(id, props(metadata));
}

#[test]
fn selector_labels_prefer_media_titles_then_names_and_ignore_blank_metadata() {
    for (class, discover) in SELECTORS {
        for (title, name, expected) in [
            (
                "  Café — [Live]  ",
                "AudioStream",
                "Browser [Café — [Live]]",
            ),
            (" \t", "  Voice call  ", "Browser [Voice call]"),
            ("", " \t", "Browser [20]"),
        ] {
            let mut graph = Graph::default();
            named_stream(
                &mut graph,
                20,
                class,
                &[("media.title", title), ("media.name", name)],
            );
            let target = discover(&graph).remove(0);
            assert_eq!(target.display_label, expected, "{class}");
            assert_eq!(graph.resolve(&target), Ok(20));
        }
    }
}

#[test]
fn selector_labels_keep_ids_only_for_ambiguous_titles_in_the_same_selector() {
    for (class, discover) in SELECTORS {
        let mut graph = Graph::default();
        for (id, title) in [(20, "Voice call"), (21, " Voice call "), (22, "Music")] {
            named_stream(&mut graph, id, class, &[("media.title", title)]);
        }
        named_stream(
            &mut graph,
            23,
            class,
            &[("application.name", "Other"), ("media.title", "Music")],
        );
        let opposite = if class == "Stream/Input/Audio" {
            "Stream/Output/Audio"
        } else {
            "Stream/Input/Audio"
        };
        named_stream(&mut graph, 24, opposite, &[("media.title", "Music")]);
        named_stream(
            &mut graph,
            25,
            class,
            &[("media.title", "Music"), ("media.role", "DSP")],
        );
        let labels: Vec<_> = discover(&graph)
            .into_iter()
            .map(|target| target.display_label)
            .collect();
        assert_eq!(
            labels,
            [
                "Browser [20]",
                "Browser [21]",
                "Browser [Music]",
                "Other [Music]"
            ],
            "{class}"
        );
    }
}

#[test]
fn selector_labels_resolve_collisions_between_titles_and_id_fallbacks() {
    for (class, discover) in SELECTORS {
        let mut graph = Graph::default();
        for (id, title) in [(20, "Voice call"), (21, "Voice call"), (22, "20")] {
            named_stream(&mut graph, id, class, &[("media.title", title)]);
        }
        let labels: Vec<_> = discover(&graph)
            .into_iter()
            .map(|target| target.display_label)
            .collect();
        assert_eq!(
            labels,
            ["Browser [20]", "Browser [21]", "Browser [22]"],
            "{class}"
        );
    }
}

#[test]
fn selector_title_updates_preserve_identity_and_restore_titles_after_duplicates_close() {
    for (class, discover) in SELECTORS {
        let mut graph = Graph::default();
        named_stream(&mut graph, 20, class, &[("media.title", "First title")]);
        let original = discover(&graph).remove(0);
        graph.update(20, props(&[("media.title", "New title")]));
        let renamed = discover(&graph).remove(0);
        assert_eq!(renamed.display_label, "Browser [New title]");
        assert_eq!(renamed.id, original.id);
        assert_eq!(graph.resolve(&original), Ok(20));

        named_stream(&mut graph, 21, class, &[("media.title", "New title")]);
        assert_eq!(discover(&graph)[0].display_label, "Browser [20]");
        graph.remove(21);
        assert_eq!(discover(&graph)[0].display_label, "Browser [New title]");
        graph.update(20, props(&[("media.title", "")]));
        let untitled = discover(&graph).remove(0);
        assert_eq!(untitled.display_label, "Browser [20]");
        assert_eq!(untitled.id, original.id);
    }
}

#[test]
fn selector_titles_replace_an_existing_id_suffix_on_application_names() {
    for (class, discover) in SELECTORS {
        let mut graph = Graph::default();
        named_stream(
            &mut graph,
            20,
            class,
            &[
                ("application.name", "Browser [20] — Capture audio"),
                ("media.name", "Voice call"),
            ],
        );
        assert_eq!(discover(&graph)[0].display_label, "Browser [Voice call]");
    }
}

#[test]
fn selector_labels_discard_title_metadata_omitted_from_node_info_updates() {
    for (class, discover) in SELECTORS {
        let mut graph = Graph::default();
        named_stream(
            &mut graph,
            20,
            class,
            &[("media.title", "Old title"), ("media.name", "AudioStream")],
        );
        let original = discover(&graph).remove(0);
        graph.update(20, props(&[("media.name", "New stream name")]));
        let renamed = discover(&graph).remove(0);
        assert_eq!(renamed.display_label, "Browser [New stream name]");
        assert_eq!(renamed.id, original.id);
        graph.update(20, Properties::new());
        let untitled = discover(&graph).remove(0);
        assert_eq!(untitled.display_label, "Browser [20]");
        assert_eq!(graph.resolve(&original), Ok(20));
    }
}
