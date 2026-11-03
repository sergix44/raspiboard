use super::*;
use std::{fs, path::Path};

#[test]
fn fresh_store_uses_a_category_file_with_uuidv7_ids_and_complete_entry_metadata() {
    let dir = fixture();
    let state = Store::new(dir.path()).load_or_create().unwrap();
    let path = dir.path().join("categories/default.json");
    assert!(path.exists(), "the default category must be created");
    let category: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        uuid::Uuid::parse_str(category["id"].as_str().unwrap())
            .unwrap()
            .get_version_num(),
        7
    );
    assert_eq!(category["category_name"], serde_json::Value::Null);
    assert_eq!(
        category["entries"],
        serde_json::json!([{
            "id": state.ordered_buttons()[0].id, "name": null, "full_path": null,
            "start_ms": null, "end_ms": null, "color": null,
            "relative_volume": null, "overlap": false, "hotkey": null, "sorting": 1
        }])
    );
    assert_eq!(
        uuid::Uuid::parse_str(&state.ordered_buttons()[0].id)
            .unwrap()
            .get_version_num(),
        7
    );
    assert!(!dir.path().join("buttons").exists());
}

#[test]
fn category_entries_load_in_sorting_order_and_edits_preserve_other_entries() {
    let dir = fixture();
    let first = "01a19322-a325-71a8-b5ad-76fcabdba7d8";
    let second = "01a19322-a325-71a8-b5ad-7f0fbc6dd140";
    let source = serde_json::json!({
        "id": "01a19322-a325-71a8-b5ad-6d57adb395e2", "category_name": null,
        "entries": [
            {"id":first,"name":null,"full_path":"/sounds/door.wav","start_ms":120,"end_ms":450,"color":"#aabbcc","relative_volume":0.75,"overlap":true,"hotkey":"Ctrl+D","sorting":2},
            {"id":second,"name":"Neighbor","full_path":null,"start_ms":null,"end_ms":null,"color":null,"relative_volume":null,"overlap":false,"hotkey":null,"sorting":1}
        ]
    });
    put(dir.path(), "categories/default.json", &source.to_string());
    let store = Store::new(dir.path());
    let state = store.load_or_create().unwrap();
    let mut button = state.ordered_buttons().remove(0);
    assert_eq!(button.id, second);
    button.name = Some("Updated".into());
    store.save_button(&button).unwrap();
    let updated: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("categories/default.json")).unwrap())
            .unwrap();
    assert_eq!(updated["entries"][0], source["entries"][0]);
    assert_eq!(updated["entries"][1]["name"], "Updated");
}

const FIRST: &str = "01a19322-a325-71a8-b5ad-76fcabdba7d8";
const SECOND: &str = "01a19322-a325-71a8-b5ad-7f0fbc6dd140";
const CATEGORY: &str = "01a19322-a325-71a8-b5ad-6d57adb395e2";
const OTHER_CATEGORY: &str = "01a19322-a325-71a8-b5ad-6d57adb395e3";

fn fixture() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-results/persistence");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

#[test]
fn instance_lock_blocks_a_second_owner_until_the_first_exits() {
    let dir = fixture();
    let store = Store::new(dir.path().join("new-store"));
    let lock = store
        .try_lock()
        .unwrap()
        .expect("first owner acquires lock");
    assert!(store.try_lock().unwrap().is_none());
    drop(lock);
    assert!(store.try_lock().unwrap().is_some());
}

#[test]
fn leftover_instance_lock_file_does_not_block_startup_or_get_truncated() {
    let dir = fixture();
    let path = dir.path().join("instance.lock");
    fs::write(&path, "leftover").unwrap();
    let store = Store::new(dir.path());
    let lock = store
        .try_lock()
        .unwrap()
        .expect("unlocked file is reusable");
    drop(lock);
    assert_eq!(fs::read_to_string(path).unwrap(), "leftover");
}

#[test]
fn instance_lock_reports_filesystem_errors_instead_of_another_owner() {
    let dir = fixture();
    let root = dir.path().join("not-a-directory");
    fs::write(&root, "keep").unwrap();
    let error = Store::new(&root).try_lock().unwrap_err();
    assert!(error.contains(&root.display().to_string()), "{error}");

    let path = dir.path().join("instance.lock");
    fs::create_dir(&path).unwrap();
    let error = Store::new(dir.path()).try_lock().unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
}

fn put(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read_category(root: &Path, filename: &str) -> Category {
    serde_json::from_slice(&fs::read(root.join("categories").join(filename)).unwrap()).unwrap()
}

fn put_category(root: &Path, category: &Category) {
    let filename = category.category_name.as_ref().map_or_else(
        || "default.json".into(),
        |_| format!("{}.json", category.id),
    );
    put(
        root,
        &format!("categories/{filename}"),
        &serde_json::to_string_pretty(category).unwrap(),
    );
}

fn entry(id: &str, sorting: u64) -> Button {
    Button {
        id: id.into(),
        sorting,
        ..Button::default()
    }
}

#[test]
fn reorder_buttons_moves_both_directions_with_contiguous_sorting() {
    let third = "01a19322-a325-71a8-b5ad-7f0fbc6dd141";
    for named in [false, true] {
        let dir = fixture();
        let category = Category {
            id: CATEGORY.into(),
            category_name: named.then(|| "Effects".into()),
            entries: vec![entry(FIRST, 10), entry(SECOND, 20), entry(third, 30)],
        };
        put_category(dir.path(), &category);
        let store = Store::new(dir.path());
        let filename = category.filename();
        for expected in [[SECOND, third, FIRST], [FIRST, SECOND, third]] {
            store
                .reorder_buttons(CATEGORY, &expected.map(String::from))
                .unwrap();
            let saved = read_category(dir.path(), &filename);
            assert_eq!(saved.id, CATEGORY);
            assert_eq!(saved.category_name, category.category_name);
            let mut entries = saved.entries;
            entries.sort_by_key(|button| button.sorting);
            assert_eq!(
                entries
                    .iter()
                    .map(|button| button.id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                entries
                    .iter()
                    .map(|button| button.sorting)
                    .collect::<Vec<_>>(),
                [1, 2, 3]
            );
        }
    }
}

#[test]
fn reorder_buttons_rejects_invalid_or_incomplete_membership_without_writing() {
    let dir = fixture();
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![entry(FIRST, 1), entry(SECOND, 2)],
        },
    );
    let other_entry = "01a19322-a325-71a8-b5ad-7f0fbc6dd141";
    put_category(
        dir.path(),
        &Category {
            id: OTHER_CATEGORY.into(),
            category_name: Some("Other".into()),
            entries: vec![entry(other_entry, 1)],
        },
    );
    let default_path = dir.path().join("categories/default.json");
    let other_path = dir.path().join(format!("categories/{OTHER_CATEGORY}.json"));
    let before = fs::read(&default_path).unwrap();
    let other_before = fs::read(&other_path).unwrap();
    let store = Store::new(dir.path());
    for (category_id, ids) in [
        (CATEGORY, vec![FIRST, FIRST]),
        (CATEGORY, vec![FIRST]),
        (CATEGORY, vec![FIRST, other_entry]),
        (CATEGORY, vec![FIRST, SECOND, other_entry]),
        (CATEGORY, vec![]),
        (CATEGORY, vec![FIRST, "../escape"]),
        (
            CATEGORY,
            vec![FIRST, "01A19322-A325-71A8-B5AD-7F0FBC6DD140"],
        ),
        ("../escape", vec![FIRST, SECOND]),
        ("01A19322-A325-71A8-B5AD-6D57ADB395E2", vec![FIRST, SECOND]),
        ("01a19322-a325-71a8-b5ad-6d57adb395e4", vec![FIRST, SECOND]),
    ] {
        let ids = ids.into_iter().map(String::from).collect::<Vec<_>>();
        assert!(
            store.reorder_buttons(category_id, &ids).is_err(),
            "accepted {category_id}: {ids:?}"
        );
        assert_eq!(fs::read(&default_path).unwrap(), before);
        assert_eq!(fs::read(&other_path).unwrap(), other_before);
    }
    assert!(!dir.path().join("settings.toml").exists());
}

#[test]
fn reorder_buttons_preserves_fresh_metadata_and_other_category_bytes() {
    let dir = fixture();
    let mut category = Category {
        id: CATEGORY.into(),
        category_name: None,
        entries: vec![entry(FIRST, 1), entry(SECOND, 2)],
    };
    put_category(dir.path(), &category);
    let store = Store::new(dir.path());
    let stale_state = store.load_or_create().unwrap();
    category.entries[0] = Button {
        name: Some("Fresh name".into()),
        full_path: Some(dir.path().join("fresh.wav")),
        start_ms: Some(100),
        end_ms: Some(500),
        color: Some("#aabbcc".into()),
        relative_volume: Some(0.75),
        overlap: true,
        hotkey: Some("Ctrl+F1".into()),
        ..category.entries[0].clone()
    };
    put_category(dir.path(), &category);
    put_category(
        dir.path(),
        &Category {
            id: OTHER_CATEGORY.into(),
            category_name: Some("Other".into()),
            entries: vec![],
        },
    );
    let other_path = dir.path().join(format!("categories/{OTHER_CATEGORY}.json"));
    let other_before = fs::read(&other_path).unwrap();
    let source_path = dir.path().join("fresh.wav");
    fs::write(&source_path, b"source audio bytes").unwrap();
    assert_eq!(stale_state.ordered_buttons()[0].name, None);
    store
        .reorder_buttons(CATEGORY, &[SECOND.into(), FIRST.into()])
        .unwrap();
    let saved = read_category(dir.path(), "default.json");
    for mut expected in category.entries {
        expected.sorting = if expected.id == FIRST { 2 } else { 1 };
        assert_eq!(
            saved.entries.iter().find(|button| button.id == expected.id),
            Some(&expected)
        );
    }
    assert_eq!(fs::read(other_path).unwrap(), other_before);
    assert_eq!(fs::read(source_path).unwrap(), b"source audio bytes");
}

#[test]
fn reorder_buttons_validates_every_category_before_writing() {
    let dir = fixture();
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![entry(FIRST, 1), entry(SECOND, 2)],
        },
    );
    put(
        dir.path(),
        "categories/broken.json",
        "preserve broken bytes",
    );
    let path = dir.path().join("categories/default.json");
    let before = fs::read(&path).unwrap();
    assert!(
        Store::new(dir.path())
            .reorder_buttons(CATEGORY, &[SECOND.into(), FIRST.into()])
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), before);
    assert_eq!(
        fs::read_to_string(dir.path().join("categories/broken.json")).unwrap(),
        "preserve broken bytes"
    );
}

#[test]
fn reorder_buttons_skips_writes_for_correct_order_and_empty_categories() {
    for entries in [vec![entry(SECOND, 2), entry(FIRST, 1)], vec![]] {
        let dir = fixture();
        let ids = if entries.is_empty() {
            vec![]
        } else {
            vec![FIRST.into(), SECOND.into()]
        };
        let text = serde_json::to_string(&Category {
            id: CATEGORY.into(),
            category_name: None,
            entries,
        })
        .unwrap();
        put(dir.path(), "categories/default.json", &text);
        Store::new(dir.path())
            .reorder_buttons(CATEGORY, &ids)
            .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("categories/default.json")).unwrap(),
            text
        );
    }
}

#[test]
fn writer_commits_reorders_in_queue_order_after_metadata_updates() {
    let dir = fixture();
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![entry(FIRST, 1), entry(SECOND, 2)],
        },
    );
    let mut writer = Writer::start(Store::new(dir.path())).unwrap();
    let mut edited = entry(FIRST, 1);
    edited.name = Some("Queued edit".into());
    drop(writer.queue(Change::Button(edited.clone())));
    drop(writer.queue(Change::ReorderButtons {
        category_id: CATEGORY.into(),
        ids: vec![SECOND.into(), FIRST.into()],
    }));
    let report = iced::futures::executor::block_on(writer.queue(Change::ReorderButtons {
        category_id: CATEGORY.into(),
        ids: vec![FIRST.into(), SECOND.into()],
    }));
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.revision, 3);
    drop(writer);
    let saved = read_category(dir.path(), "default.json");
    assert_eq!(
        saved.entries.iter().find(|button| button.id == FIRST),
        Some(&edited)
    );
    assert_eq!(
        saved.entries.iter().find(|button| button.id == SECOND),
        Some(&entry(SECOND, 2))
    );
}

#[test]
fn writer_retains_reorder_errors_until_that_category_reorders_successfully() {
    let dir = fixture();
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![entry(FIRST, 1), entry(SECOND, 2)],
        },
    );
    put_category(
        dir.path(),
        &Category {
            id: OTHER_CATEGORY.into(),
            category_name: Some("Other".into()),
            entries: vec![],
        },
    );
    let mut writer = Writer::start(Store::new(dir.path())).unwrap();
    let failed = iced::futures::executor::block_on(writer.queue(Change::ReorderButtons {
        category_id: CATEGORY.into(),
        ids: vec![FIRST.into()],
    }));
    assert_eq!(failed.errors.len(), 1);
    let unrelated = iced::futures::executor::block_on(writer.queue(Change::ReorderButtons {
        category_id: OTHER_CATEGORY.into(),
        ids: vec![],
    }));
    assert_eq!(unrelated.errors, failed.errors);
    let recovered = iced::futures::executor::block_on(writer.queue(Change::ReorderButtons {
        category_id: CATEGORY.into(),
        ids: vec![SECOND.into(), FIRST.into()],
    }));
    assert!(recovered.errors.is_empty(), "{:?}", recovered.errors);
}

#[test]
fn empty_store_initializes_reloadable_defaults_with_deck_theme() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let state = store.load_or_create().unwrap();
    assert_eq!(state.settings, Settings::default());
    assert_eq!(state.ordered_buttons().len(), 1);
    assert_eq!(store.load_or_create().unwrap(), state);
    assert!(
        fs::read_to_string(dir.path().join("settings.toml"))
            .unwrap()
            .contains("theme = \"Deck\"")
    );
    assert!(!dir.path().join("grid.json").exists());
    let category = read_category(dir.path(), "default.json");
    assert_ne!(category.id, state.ordered_buttons()[0].id);
    assert_eq!(category.entries, state.ordered_buttons());
}

#[test]
fn explicitly_empty_category_stays_empty() {
    let dir = fixture();
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![],
        },
    );
    let store = Store::new(dir.path());
    assert!(store.load_or_create().unwrap().ordered_buttons().is_empty());
    assert!(store.load_or_create().unwrap().ordered_buttons().is_empty());
}

#[test]
fn metadata_roundtrips_unicode_names_and_absolute_missing_audio_paths() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let state = store.load_or_create().unwrap();
    let button = Button {
        name: Some("Applausi — caffè ☕".into()),
        full_path: Some(dir.path().join("Music/è un suono.wav")),
        start_ms: Some(100),
        end_ms: Some(350),
        color: Some("#abcdef".into()),
        relative_volume: Some(0.75),
        overlap: true,
        hotkey: Some("Ctrl+F1".into()),
        ..state.ordered_buttons()[0].clone()
    };
    store.save_button(&button).unwrap();
    assert_eq!(store.load_or_create().unwrap().ordered_buttons(), [button]);
    assert!(!dir.path().join("Music").exists());
}

#[test]
fn named_categories_load_after_default_and_entry_edits_preserve_category_ownership() {
    let dir = fixture();
    let default = Category {
        id: CATEGORY.into(),
        category_name: None,
        entries: vec![entry(FIRST, 4)],
    };
    let named = Category {
        id: OTHER_CATEGORY.into(),
        category_name: Some("Effects".into()),
        entries: vec![entry(SECOND, 1)],
    };
    put_category(dir.path(), &default);
    put_category(dir.path(), &named);
    let default_bytes = fs::read(dir.path().join("categories/default.json")).unwrap();
    let store = Store::new(dir.path());
    let state = store.load_or_create().unwrap();
    assert_eq!(
        state
            .ordered_buttons()
            .iter()
            .map(|b| b.id.as_str())
            .collect::<Vec<_>>(),
        [FIRST, SECOND]
    );
    let mut button = state.ordered_buttons().remove(1);
    button.name = Some("Updated".into());
    store.save_button(&button).unwrap();
    assert_eq!(
        fs::read(dir.path().join("categories/default.json")).unwrap(),
        default_bytes
    );
    let updated = read_category(dir.path(), &format!("{OTHER_CATEGORY}.json"));
    assert_eq!(updated.category_name, named.category_name);
    assert_eq!(updated.entries, [button]);
}

#[test]
fn missing_default_category_is_created_without_adding_an_unassigned_entry() {
    let dir = fixture();
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: Some("Effects".into()),
            entries: vec![entry(FIRST, 1)],
        },
    );
    let state = Store::new(dir.path()).load_or_create().unwrap();
    assert_eq!(state.ordered_buttons().len(), 1);
    assert_eq!(state.ordered_buttons()[0].id, FIRST);
    assert!(read_category(dir.path(), "default.json").entries.is_empty());
}

#[test]
fn sorting_is_numeric_and_ties_keep_their_array_order() {
    let dir = fixture();
    let third = Button {
        sorting: 2,
        ..Button::default()
    };
    put_category(
        dir.path(),
        &Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![entry(FIRST, 10), entry(SECOND, 2), third.clone()],
        },
    );
    let state = Store::new(dir.path()).load_or_create().unwrap();
    assert_eq!(
        state
            .ordered_buttons()
            .iter()
            .map(|b| b.id.as_str())
            .collect::<Vec<_>>(),
        [SECOND, third.id.as_str(), FIRST]
    );
}

#[test]
fn adding_entries_appends_to_default_and_never_creates_a_button_or_grid_file() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let state = store.load_or_create().unwrap();
    let button = entry(SECOND, 2);
    let mut writer = Writer::start(store.clone()).unwrap();
    let report = iced::futures::executor::block_on(writer.queue(Change::AddButton(button.clone())));
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    drop(writer);
    assert_eq!(
        store.load_or_create().unwrap().ordered_buttons(),
        [state.ordered_buttons()[0].clone(), button]
    );
    assert_eq!(
        fs::read_dir(dir.path().join("categories")).unwrap().count(),
        1
    );
    assert!(!dir.path().join("buttons").exists());
    assert!(!dir.path().join("grid.json").exists());
}

#[test]
fn adding_a_duplicate_never_overwrites_existing_entries() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let original = store.load_or_create().unwrap();
    let mut replacement = original.ordered_buttons()[0].clone();
    replacement.name = Some("Replacement".into());
    let before = fs::read(dir.path().join("categories/default.json")).unwrap();
    let mut writer = Writer::start(store.clone()).unwrap();
    let report = iced::futures::executor::block_on(writer.queue(Change::AddButton(replacement)));
    assert!(!report.errors.is_empty());
    drop(writer);
    assert_eq!(
        fs::read(dir.path().join("categories/default.json")).unwrap(),
        before
    );
    assert_eq!(store.load_or_create().unwrap(), original);
}

#[test]
fn invalid_category_files_are_preserved_before_initialization() {
    let valid = serde_json::json!({"id":CATEGORY,"category_name":null,"entries":[{"id":FIRST,"sorting":1}]});
    let mut cases = vec!["broken json".to_owned()];
    for (field, value) in [
        ("id", serde_json::json!("not-a-uuid")),
        (
            "id",
            serde_json::json!("01a19322-a325-41a8-b5ad-6d57adb395e2"),
        ),
        ("category_name", serde_json::json!("Named")),
        ("unknown", serde_json::json!(42)),
    ] {
        let mut category = valid.clone();
        category[field] = value;
        cases.push(category.to_string());
    }
    for (field, value) in [
        ("id", serde_json::json!("../escape")),
        (
            "id",
            serde_json::json!("01a19322-a325-41a8-b5ad-76fcabdba7d8"),
        ),
        ("full_path", serde_json::json!("relative.wav")),
        ("sorting", serde_json::json!(-1)),
        ("sorting", serde_json::json!("first")),
        ("overlap", serde_json::Value::Null),
        ("extra_metadata", serde_json::json!(42)),
    ] {
        let mut category = valid.clone();
        category["entries"][0][field] = value;
        cases.push(category.to_string());
    }
    for text in cases {
        let dir = fixture();
        put(dir.path(), "categories/default.json", &text);
        let error = Store::new(dir.path()).load_or_create().unwrap_err();
        assert!(error.contains("categories/default.json"), "{error}");
        assert_eq!(
            fs::read_to_string(dir.path().join("categories/default.json")).unwrap(),
            text
        );
        assert!(!dir.path().join("settings.toml").exists());
    }
}

#[test]
fn duplicate_entry_ids_within_or_between_categories_are_rejected() {
    for same_category in [true, false] {
        let dir = fixture();
        let mut default = Category {
            id: CATEGORY.into(),
            category_name: None,
            entries: vec![entry(FIRST, 1)],
        };
        if same_category {
            default.entries.push(entry(FIRST, 2));
        } else {
            put_category(
                dir.path(),
                &Category {
                    id: OTHER_CATEGORY.into(),
                    category_name: Some("Effects".into()),
                    entries: vec![entry(FIRST, 2)],
                },
            );
        }
        put_category(dir.path(), &default);
        assert!(
            Store::new(dir.path())
                .load_or_create()
                .unwrap_err()
                .contains("duplicate")
        );
        assert!(!dir.path().join("settings.toml").exists());
    }
}

#[test]
fn invalid_named_category_filename_and_multiple_defaults_are_rejected() {
    for name in [Some("Effects"), None] {
        let dir = fixture();
        put_category(
            dir.path(),
            &Category {
                id: CATEGORY.into(),
                category_name: None,
                entries: vec![],
            },
        );
        let extra = serde_json::json!({"id":OTHER_CATEGORY,"category_name":name,"entries":[]});
        put(dir.path(), "categories/wrong.json", &extra.to_string());
        assert!(
            Store::new(dir.path())
                .load_or_create()
                .unwrap_err()
                .contains("wrong.json")
        );
        assert!(!dir.path().join("settings.toml").exists());
    }
}

#[test]
fn corrupt_unreferenced_category_prevents_any_save_or_initialization() {
    let dir = fixture();
    let valid = Category {
        id: CATEGORY.into(),
        category_name: None,
        entries: vec![entry(FIRST, 1)],
    };
    put_category(dir.path(), &valid);
    put(dir.path(), "categories/broken.json", "preserve me");
    let before = fs::read(dir.path().join("categories/default.json")).unwrap();
    let store = Store::new(dir.path());
    assert!(store.load_or_create().is_err());
    assert!(store.save_button(&entry(FIRST, 1)).is_err());
    assert_eq!(
        fs::read(dir.path().join("categories/default.json")).unwrap(),
        before
    );
    assert!(!dir.path().join("settings.toml").exists());
}

#[test]
fn a_categories_file_is_not_replaced_by_a_directory_or_settings() {
    let dir = fixture();
    put(dir.path(), "categories", "preserve me");
    assert!(Store::new(dir.path()).load_or_create().is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("categories")).unwrap(),
        "preserve me"
    );
    assert!(!dir.path().join("settings.toml").exists());
}

#[test]
fn invalid_writes_preserve_existing_category_bytes() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let original = store.load_or_create().unwrap();
    let before = fs::read(dir.path().join("categories/default.json")).unwrap();
    for id in [
        "../escape",
        "",
        "button-1",
        "01a19322-a325-41a8-b5ad-76fcabdba7d8",
    ] {
        let mut button = original.ordered_buttons()[0].clone();
        button.id = id.into();
        assert!(store.save_button(&button).is_err());
    }
    let mut button = original.ordered_buttons()[0].clone();
    button.full_path = Some("relative.wav".into());
    assert!(store.save_button(&button).is_err());
    assert_eq!(
        fs::read(dir.path().join("categories/default.json")).unwrap(),
        before
    );
}

#[test]
fn writer_drains_multiple_entry_updates_without_losing_neighbors() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let initial = store.load_or_create().unwrap();
    let mut writer = Writer::start(store.clone()).unwrap();
    for (id, sorting) in [(FIRST, 2), (SECOND, 3)] {
        drop(writer.queue(Change::AddButton(entry(id, sorting))));
    }
    let mut edited = initial.ordered_buttons()[0].clone();
    edited.name = Some("First".into());
    drop(writer.queue(Change::Button(edited.clone())));
    drop(writer);
    assert_eq!(
        store.load_or_create().unwrap().ordered_buttons(),
        [edited, entry(FIRST, 2), entry(SECOND, 3)]
    );
}

#[test]
fn writer_retains_failures_until_the_same_entry_saves_successfully() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let initial = store.load_or_create().unwrap();
    let path = dir.path().join("categories/default.json");
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, "broken json").unwrap();
    let mut writer = Writer::start(store.clone()).unwrap();
    let failed = iced::futures::executor::block_on(
        writer.queue(Change::Button(initial.ordered_buttons()[0].clone())),
    );
    assert_eq!(failed.errors.len(), 1);
    let settings =
        iced::futures::executor::block_on(writer.queue(Change::Settings(Settings::default())));
    assert_eq!(settings.errors, failed.errors);
    fs::write(path, bytes).unwrap();
    let recovered = iced::futures::executor::block_on(
        writer.queue(Change::Button(initial.ordered_buttons()[0].clone())),
    );
    assert!(recovered.errors.is_empty(), "{:?}", recovered.errors);
}

#[test]
fn saving_another_entry_never_hides_an_unsaved_edit() {
    for same_category in [true, false] {
        let dir = fixture();
        let store = Store::new(dir.path());
        let initial = store.load_or_create().unwrap();
        let mut first = initial.ordered_buttons().remove(0);
        let mut second = entry(SECOND, 2);
        if same_category {
            store.save_button(&second).unwrap();
        } else {
            put_category(
                dir.path(),
                &Category {
                    id: OTHER_CATEGORY.into(),
                    category_name: Some("Effects".into()),
                    entries: vec![second.clone()],
                },
            );
        }
        let path = dir.path().join("categories/default.json");
        let original_bytes = fs::read(&path).unwrap();
        fs::write(&path, "broken json").unwrap();
        first.full_path = Some(dir.path().join("replacement.wav"));
        let mut writer = Writer::start(store.clone()).unwrap();
        let failed = iced::futures::executor::block_on(writer.queue(Change::Button(first.clone())));
        assert_eq!(failed.errors.len(), 1);
        fs::write(&path, original_bytes).unwrap();
        second.name = Some("Saved neighbor".into());
        let neighbor =
            iced::futures::executor::block_on(writer.queue(Change::Button(second.clone())));
        assert_eq!(
            neighbor.errors, failed.errors,
            "an unrelated save must retain the failed edit"
        );
        let on_disk = store.load_or_create().unwrap().ordered_buttons();
        assert_eq!(on_disk[0].full_path, None);
        assert_eq!(on_disk[1], second);
        let recovered =
            iced::futures::executor::block_on(writer.queue(Change::Button(first.clone())));
        assert!(recovered.errors.is_empty());
        drop(writer);
        assert_eq!(store.load_or_create().unwrap().ordered_buttons()[0], first);
    }
}

#[test]
fn empty_or_null_names_fall_back_to_the_filename() {
    let mut button = Button {
        full_path: Some("/sounds/door slam.wav".into()),
        ..Button::default()
    };
    for name in [None, Some("".into()), Some("   ".into())] {
        button.name = name;
        assert_eq!(button.display_name(), "door slam");
    }
    button.name = Some("Door".into());
    assert_eq!(button.display_name(), "Door");
    button.name = None;
    button.full_path = None;
    assert_eq!(button.display_name(), "Choose sound");
}

#[test]
fn malformed_settings_are_preserved_before_any_missing_defaults_are_created() {
    let dir = fixture();
    put(dir.path(), "settings.toml", "remote_volume = [broken");
    let result = Store::new(dir.path()).load_or_create();
    assert!(result.unwrap_err().contains("settings.toml"));
    assert_eq!(
        fs::read_to_string(dir.path().join("settings.toml")).unwrap(),
        "remote_volume = [broken"
    );
    assert!(!dir.path().join("grid.json").exists());
    assert!(!dir.path().join("categories").exists());
}

#[test]
fn unknown_settings_fields_report_the_path_and_preserve_bytes_without_defaults() {
    let dir = fixture();
    let text = "remote_volum = 37\n";
    put(dir.path(), "settings.toml", text);
    let error = Store::new(dir.path()).load_or_create().unwrap_err();
    assert!(error.contains(dir.path().join("settings.toml").to_str().unwrap()));
    assert!(error.contains("remote_volum"));
    assert_eq!(
        fs::read_to_string(dir.path().join("settings.toml")).unwrap(),
        text
    );
    assert!(!dir.path().join("grid.json").exists());
    assert!(!dir.path().join("categories").exists());
}

#[test]
fn omitted_settings_fields_keep_their_defaults() {
    let dir = fixture();
    put(dir.path(), "settings.toml", "remote_volume = 37\n");
    let state = Store::new(dir.path()).load_or_create().unwrap();
    assert_eq!(state.settings.remote_volume, 37);
    assert_eq!(state.settings.sound_button_scale, 100);
    assert_eq!(state.settings.local_volume, 100);
    assert!(!state.settings.volumes_linked);
    assert_eq!(state.settings.theme, ThemeChoice::Deck);
    assert_eq!(
        fs::read_to_string(dir.path().join("settings.toml")).unwrap(),
        "remote_volume = 37\n"
    );
}

#[test]
fn load_rejects_invalid_settings_without_creating_other_files() {
    for text in [
        "remote_volume = 101",
        "sound_button_scale = 49",
        "sound_button_scale = 201",
        "local_volume = -1",
        "remote_volume = 256",
        "remote_volume = 20\nlocal_volume = 21\nvolumes_linked = true",
        "theme = 'Unknown theme'",
        "theme = 'Dark'",
        "theme = 'deck'",
        "theme = 'console'",
        "theme = 0",
        "theme = []",
        "theme = 'Console'\ntheme_extra = true",
    ] {
        let dir = fixture();
        put(dir.path(), "settings.toml", text);
        assert!(
            Store::new(dir.path()).load_or_create().is_err(),
            "accepted {text}"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.toml")).unwrap(),
            text
        );
        assert!(!dir.path().join("grid.json").exists());
    }
}

#[test]
fn initializing_a_missing_file_never_clobbers_a_destination_that_appeared() {
    let dir = fixture();
    put(dir.path(), "settings.toml", "existing bytes");
    assert!(write_atomic(&dir.path().join("settings.toml"), b"defaults", true).is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("settings.toml")).unwrap(),
        "existing bytes"
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn every_application_theme_roundtrips_with_its_exact_name() {
    let dir = fixture();
    let store = Store::new(dir.path());
    store.load_or_create().unwrap();
    for &theme in ThemeChoice::ALL {
        let name = match theme {
            ThemeChoice::Deck => "Deck",
            ThemeChoice::Console => "Console",
        };
        store
            .save_settings(&Settings {
                theme,
                ..Settings::default()
            })
            .unwrap();
        assert_eq!(store.load_or_create().unwrap().settings.theme, theme);
        let saved: toml::Value =
            toml::from_str(&fs::read_to_string(dir.path().join("settings.toml")).unwrap()).unwrap();
        assert_eq!(saved["theme"].as_str(), Some(name));
    }
}

#[test]
fn all_native_theme_names_are_rejected_without_rewriting_settings() {
    for theme in iced::Theme::ALL {
        let dir = fixture();
        let text = format!("theme = {:?}\n", theme.to_string());
        put(dir.path(), "settings.toml", &text);
        let error = Store::new(dir.path()).load_or_create().unwrap_err();
        assert!(error.contains("settings.toml"), "{error}");
        assert_eq!(
            fs::read_to_string(dir.path().join("settings.toml")).unwrap(),
            text
        );
        assert!(!dir.path().join("categories").exists());
    }
}

#[test]
fn writer_saves_enqueued_edits_in_order_and_drains_canceled_receivers_on_drop() {
    let dir = fixture();
    let store = Store::new(dir.path());
    store.load_or_create().unwrap();
    let mut writer = Writer::start(store.clone()).unwrap();
    let first = writer.queue(Change::Settings(Settings {
        remote_volume: 10,
        ..Settings::default()
    }));
    let second = writer.queue(Change::Settings(Settings {
        remote_volume: 20,
        ..Settings::default()
    }));
    let third = writer.queue(Change::Settings(Settings {
        remote_volume: 30,
        ..Settings::default()
    }));
    let second = iced::futures::executor::block_on(second);
    let first = iced::futures::executor::block_on(first);
    assert_eq!(first.revision, 1);
    assert_eq!(second.revision, 2);
    assert!(first.errors.is_empty());
    assert!(second.errors.is_empty());
    drop(third);
    for volume in 31..=65 {
        drop(writer.queue(Change::Settings(Settings {
            remote_volume: volume,
            ..Settings::default()
        })));
    }
    drop(writer);
    assert_eq!(store.load_or_create().unwrap().settings.remote_volume, 65);
}

#[test]
fn deleting_a_button_preserves_its_category_neighbors_and_source_file() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let initial = store.load_or_create().unwrap();
    let mut button = initial.ordered_buttons()[0].clone();
    let source = dir.path().join("source.wav");
    fs::write(&source, b"original audio").unwrap();
    button.full_path = Some(source.clone());
    store.save_button(&button).unwrap();
    let other = Button {
        sorting: 2,
        ..Button::default()
    };
    store.save_button(&other).unwrap();
    let mut writer = Writer::start(store.clone()).unwrap();
    let report =
        iced::futures::executor::block_on(writer.queue(Change::DeleteButton(button.id.clone())));
    assert!(report.errors.is_empty());
    let saved = store.load_or_create().unwrap();
    assert_eq!(saved.categories[0].id, initial.categories[0].id);
    assert_eq!(saved.ordered_buttons(), vec![other]);
    assert_eq!(fs::read(source).unwrap(), b"original audio");
    let report = iced::futures::executor::block_on(writer.queue(Change::DeleteButton(button.id)));
    assert!(report.errors.is_empty());
}

#[test]
fn deleting_from_a_named_category_leaves_other_category_bytes_untouched() {
    let dir = fixture();
    let default = Category {
        id: CATEGORY.into(),
        category_name: None,
        entries: vec![entry(FIRST, 1)],
    };
    let named = Category {
        id: OTHER_CATEGORY.into(),
        category_name: Some("Effects".into()),
        entries: vec![entry(SECOND, 1)],
    };
    put_category(dir.path(), &default);
    put_category(dir.path(), &named);
    let default_path = dir.path().join("categories/default.json");
    let before = fs::read(&default_path).unwrap();
    let store = Store::new(dir.path());
    store.delete_button(SECOND).unwrap();
    assert_eq!(fs::read(default_path).unwrap(), before);
    let saved = read_category(dir.path(), &format!("{OTHER_CATEGORY}.json"));
    assert_eq!(saved.id, named.id);
    assert_eq!(saved.category_name, named.category_name);
    assert!(saved.entries.is_empty());
}

#[test]
fn invalid_sound_options_are_rejected_without_rewriting_saved_metadata() {
    let dir = fixture();
    let store = Store::new(dir.path());
    let saved = store.load_or_create().unwrap().ordered_buttons()[0].clone();
    let path = dir.path().join("categories/default.json");
    let before = fs::read(&path).unwrap();
    for button in [
        Button {
            color: Some("purple".into()),
            ..saved.clone()
        },
        Button {
            relative_volume: Some(16.0),
            ..saved.clone()
        },
        Button {
            start_ms: Some(200),
            end_ms: Some(100),
            ..saved.clone()
        },
    ] {
        assert!(store.save_button(&button).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
