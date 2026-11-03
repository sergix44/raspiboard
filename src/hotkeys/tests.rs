use super::*;

#[test]
fn only_current_session_members_activate_once_until_released() {
    let mut keys = ActivationState::new("/session/current".into(), ["horn".into()]);
    assert!(!keys.press("/session/old", "horn"));
    assert!(!keys.press("/session/current", "deleted"));
    assert!(keys.press("/session/current", "horn"));
    assert!(!keys.press("/session/current", "horn"));
    keys.release("/session/old", "horn");
    assert!(!keys.press("/session/current", "horn"));
    keys.release("/session/current", "horn");
    assert!(keys.press("/session/current", "horn"));
}

#[test]
fn desktop_assignments_use_confirmed_descriptions_and_normalize_empty_values() {
    let shortcuts: Vec<Shortcut> = serde_json::from_str(
        r#"[
        ["horn", {"description": ["s", "Horn"], "trigger_description": ["s", "Ctrl+F1"]}],
        ["bell", {"description": ["s", "Bell"], "trigger_description": ["s", "   "]}]
    ]"#,
    )
    .unwrap();
    assert_eq!(
        assignments(&shortcuts),
        vec![
            Assignment {
                id: "horn".into(),
                trigger: Some("Ctrl+F1".into())
            },
            Assignment {
                id: "bell".into(),
                trigger: None
            },
        ]
    );
}

#[test]
fn successful_binding_disables_omitted_actions_but_partial_changes_do_not() {
    let shortcuts: Vec<Shortcut> = serde_json::from_str(
        r#"[
        ["horn", {"description": ["s", "Horn"], "trigger_description": ["s", "Alt+H"]}]
    ]"#,
    )
    .unwrap();
    let config = Config {
        bindings: vec![
            Binding {
                id: "horn".into(),
                description: "Play Horn".into(),
            },
            Binding {
                id: "bell".into(),
                description: "Play Bell".into(),
            },
        ],
        ..Config::default()
    };
    assert_eq!(
        bound_assignments(&config, &shortcuts),
        vec![
            Assignment {
                id: "horn".into(),
                trigger: Some("Alt+H".into())
            },
            Assignment {
                id: "bell".into(),
                trigger: None
            },
        ]
    );
    assert_eq!(
        assignments(&shortcuts),
        vec![Assignment {
            id: "horn".into(),
            trigger: Some("Alt+H".into())
        },]
    );
}

#[test]
fn queued_overlapping_keys_keep_their_press_and_release_order() {
    let messages = [
        ("Activated", "horn"),
        ("Activated", "bell"),
        ("Deactivated", "bell"),
        ("Deactivated", "horn"),
        ("Activated", "bell"),
    ]
    .into_iter()
    .map(|(kind, id)| {
        zbus::Message::signal(
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.GlobalShortcuts",
            kind,
        )
        .unwrap()
        .build(&(
            ashpd::zvariant::ObjectPath::try_from("/session/current").unwrap(),
            id,
            1_u64,
            std::collections::HashMap::<String, ashpd::zvariant::OwnedValue>::new(),
        ))
        .unwrap()
    });
    let events = iced::futures::executor::block_on(
        portal_signals(stream::iter(messages)).collect::<Vec<_>>(),
    );
    let mut keys = ActivationState::new("/session/current".into(), ["horn".into(), "bell".into()]);
    let mut played = Vec::new();
    for event in events {
        match event.unwrap() {
            Signal::Pressed(event)
                if keys.press(event.session_handle().as_str(), event.shortcut_id()) =>
            {
                played.push(event.shortcut_id().to_owned())
            }
            Signal::Released(event) => {
                keys.release(event.session_handle().as_str(), event.shortcut_id())
            }
            _ => {}
        }
    }
    assert_eq!(played, ["horn", "bell", "bell"]);
}
