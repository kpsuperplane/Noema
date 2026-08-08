use super::*;
use tempfile::tempdir;

#[test]
fn native_memory_initializes_root_and_rebuilds_search_index() {
    let directory = tempdir().expect("tempdir");
    let index_path = directory.path().join("system/indexes/memory.sqlite3");
    std::fs::create_dir_all(index_path.parent().expect("index directory"))
        .expect("create index directory");
    let legacy_index = Connection::open(&index_path).expect("legacy index");
    legacy_index
        .execute_batch(
            "CREATE TABLE memory_pages (path TEXT PRIMARY KEY, id TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL, hash TEXT NOT NULL);
             INSERT INTO memory_pages VALUES ('stale.md', 'memory:human:stale.md', 'Stale', 'obsolete private body', 'old');",
        )
        .expect("legacy index rows");
    drop(legacy_index);
    let memory = NativeMemory::new(
        directory.path().join("memory/human"),
        &index_path,
    );
    memory.initialize().expect("initialize");
    let rebuilt_index = Connection::open(&index_path).expect("rebuilt index");
    let legacy_table_count: i64 = rebuilt_index
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'memory_pages'",
            [],
            |row| row.get(0),
        )
        .expect("legacy table count");
    assert_eq!(legacy_table_count, 0);
    memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "people.md".into(),
                title: "People".into(),
                icon: "users".into(),
                body: "Alice recently completed several hikes and likes tea".into(),
                sources: vec![],
            }],
            deletes: vec![],
        })
        .expect("publish");
    let child = &memory.read_root().expect("root").children[0];
    assert_eq!(child.path, "people.md");
    assert_eq!(child.icon, "users");
    assert_eq!(
        child.excerpt,
        "Alice recently completed several hikes and likes tea"
    );
    let people_path = directory.path().join("memory/human/people.md");
    assert!(
        std::fs::read_to_string(&people_path)
            .expect("canonical child")
            .contains("\nicon: users\n")
    );
    assert_eq!(
        memory.search("Alice", 5).expect("search")[0].path,
        "people.md"
    );
    assert_eq!(
        memory.search("Alice \"", 5).expect("punctuated search")[0].path,
        "people.md"
    );
    assert_eq!(
        memory
            .search("recent hike completed", 5)
            .expect("inflected search")[0]
            .path,
        "people.md"
    );
    assert!(memory.search("unrelated Alice", 5).expect("strict search").is_empty());
    assert_eq!(
        memory
            .search_relevant("unrelated conversation context Alice", 5)
            .expect("relevance search")[0]
            .path,
        "people.md"
    );
    let long_query = (0..140)
        .map(|index| format!("term{index}"))
        .chain(["Alice".to_string()])
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(memory.search_relevant(&long_query, 5).expect("long relevance query")[0].path, "people.md");
    assert_eq!(
        memory.read_page("people.md").expect("page").body,
        "Alice recently completed several hikes and likes tea"
    );
    assert_eq!(
        memory
            .read_page("memory:human:people.md")
            .expect("page by stable id")
            .path,
        "people.md"
    );

    let legacy_child = "---\nschema: noema.memory.page/v1\nid: memory:human:people.md\nowner: human:local\nscope: human:local\ntitle: People\nsources:\n---\n\n# People\n\nAlice likes tea";
    std::fs::write(&people_path, legacy_child).expect("write legacy child");
    let legacy_child_page = memory.read_page("people.md").expect("legacy child");
    assert_eq!(legacy_child_page.icon, "file-text");
    assert_eq!(legacy_child_page.hash, hash_content(legacy_child.as_bytes()));
    assert_eq!(std::fs::read_to_string(people_path).expect("legacy bytes"), legacy_child);

    let legacy_root = format!(
        "---\nschema: noema.memory.page/v1\nid: memory:human:root.md\nowner: human:local\nscope: human:local\ntitle: Kevin\nsources:\n---\n\n# Kevin\n\n{}",
        "Kevin has a durable preference for thoughtful technical systems. ".repeat(24)
    );
    let root_path = directory.path().join("memory/human/root.md");
    std::fs::write(&root_path, &legacy_root).expect("write legacy root inventory");
    let legacy_page = memory.read_root().expect("legacy root");
    assert_eq!(legacy_page.icon, "user");
    assert_eq!(legacy_page.hash, hash_content(legacy_root.as_bytes()));
    assert_eq!(std::fs::read_to_string(root_path).expect("legacy bytes"), legacy_root);
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "interests.md".into(),
                title: "Interests".into(),
                icon: "sparkles".into(),
                body: "Kevin enjoys hiking.".into(),
                sources: vec![],
            }],
            deletes: vec![],
        })
        .is_err());
}

#[test]
fn native_memory_rejects_unsafe_paths_and_oversized_bodies() {
    assert!(normalize_page_path("../root.md").is_err());
    assert!(normalize_page_path(".pending/injected.md").is_err());
    assert!(validate_page_content(&"word ".repeat(MEMORY_MAX_WORDS + 1)).is_err());
    assert!(validate_change_set(&MemoryChangeSet {
        upserts: vec![MemoryPageChange {
            id: None,
            expected_hash: None,
            path: "injected.md".into(),
            title: "Title\nowner: attacker".into(),
            icon: "file-text".into(),
            body: String::new(),
            sources: vec![],
        }],
        deletes: vec![],
    })
    .is_err());
    assert!(validate_change_set(&MemoryChangeSet {
        upserts: vec![MemoryPageChange {
            id: None,
            expected_hash: None,
            path: "duplicate-title.md".into(),
            title: "Duplicate title".into(),
            icon: "file-text".into(),
            body: "# Duplicate title\n\nLead".into(),
            sources: vec![],
        }],
        deletes: vec![],
    })
    .is_err());
    assert!(validate_change_set(&MemoryChangeSet {
        upserts: vec![MemoryPageChange {
            id: None,
            expected_hash: None,
            path: ROOT_PAGE_PATH.into(),
            title: "Kevin".into(),
            icon: "user".into(),
            body: "Kevin has a durable preference for thoughtful technical systems. ".repeat(24),
            sources: vec![],
        }],
        deletes: vec![],
    })
    .is_err());
    assert!(validate_change_set(&MemoryChangeSet {
        upserts: vec![MemoryPageChange {
            id: None,
            expected_hash: None,
            path: "unsupported-icon.md".into(),
            title: "Unsupported icon".into(),
            icon: "not-a-lucide-icon".into(),
            body: String::new(),
            sources: vec![],
        }],
        deletes: vec![],
    })
    .is_err());
}

#[test]
fn native_memory_recovers_exact_staged_bytes_before_indexing() {
    let directory = tempdir().expect("tempdir");
    let memory = NativeMemory::new(
        directory.path().join("memory/human"),
        directory.path().join("system/indexes/memory.sqlite3"),
    );
    memory.initialize().expect("initialize");
    let change = MemoryPageChange {
        id: None,
        expected_hash: None,
        path: "people.md".into(),
        title: "People".into(),
        icon: "users".into(),
        body: "Alice".into(),
        sources: vec![],
    };
    let bytes = memory
        .render_page(&change, "people.md")
        .expect("render")
        .into_bytes();
    let pending_dir = memory.root().join(".pending/recovery");
    std::fs::create_dir_all(&pending_dir).expect("pending dir");
    let pending = PendingPayload {
        pages: vec![StagedPage {
            path: "people.md".into(),
            bytes: bytes.clone(),
        }],
        deletes: vec![],
        state: MemoryState {
            updated_at: "1".into(),
            ..MemoryState::default()
        },
    };
    std::fs::write(
        pending_dir.join("changes.json"),
        serde_json::to_vec(&pending).expect("payload"),
    )
    .expect("stage");
    memory.initialize().expect("recover");
    assert_eq!(
        std::fs::read(memory.root().join("people.md")).expect("page"),
        bytes
    );
    assert_eq!(
        memory.search("Alice", 5).expect("search")[0].path,
        "people.md"
    );
}

#[test]
fn native_memory_enforces_compare_publish_citations_and_rendered_word_limit() {
    let directory = tempdir().expect("tempdir");
    let memory = NativeMemory::new(
        directory.path().join("memory/human"),
        directory.path().join("system/indexes/memory.sqlite3"),
    );
    memory.initialize().expect("initialize");
    memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "people.md".into(),
                title: "People".into(),
                icon: "users".into(),
                body: "Alice [^fact]\n\n[^fact]: item-1".into(),
                sources: vec!["item-1".into()],
            }],
            deletes: vec![],
        })
        .expect("initial publish");
    let current = memory.read_page("people.md").expect("page");
    memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: Some(current.id.clone()),
                expected_hash: Some(current.hash.clone()),
                path: "people.md".into(),
                title: "People".into(),
                icon: "users".into(),
                body: "Updated [^fact]\n\n[^fact]: item-1".into(),
                sources: vec!["item-1".into()],
            }],
            deletes: vec![],
        })
        .expect("compare-and-publish update");
    let updated = memory.read_page("people.md").expect("updated page");
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: Some(updated.id.clone()),
                expected_hash: Some(current.hash),
                path: "people.md".into(),
                title: "People".into(),
                icon: "users".into(),
                body: "Stale [^fact]\n\n[^fact]: item-1".into(),
                sources: vec!["item-1".into()],
            }],
            deletes: vec![],
        })
        .is_err());
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: Some(updated.id),
                expected_hash: Some(updated.hash),
                path: "people.md".into(),
                title: "People".into(),
                icon: "users".into(),
                body: "Missing citation".into(),
                sources: vec!["item-1".into()],
            }],
            deletes: vec![],
        })
        .is_err());
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "too-long.md".into(),
                title: "Title".into(),
                icon: "file-text".into(),
                body: "word ".repeat(MEMORY_MAX_WORDS),
                sources: vec![],
            }],
            deletes: vec![],
        })
        .is_err());
}

#[test]
fn native_memory_validates_final_hierarchy_and_preserves_ids_across_moves() {
    let directory = tempdir().expect("tempdir");
    let memory = NativeMemory::new(
        directory.path().join("memory/human"),
        directory.path().join("system/indexes/memory.sqlite3"),
    );
    memory.initialize().expect("initialize");
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "people/alice.md".into(),
                title: "Alice".into(),
                icon: "user".into(),
                body: String::new(),
                sources: vec![],
            }],
            deletes: vec![],
        })
        .is_err());
    memory
        .publish(&MemoryChangeSet {
            upserts: vec![
                MemoryPageChange {
                    id: None,
                    expected_hash: None,
                    path: "people.md".into(),
                    title: "People".into(),
                    icon: "users".into(),
                    body: String::new(),
                    sources: vec![],
                },
                MemoryPageChange {
                    id: None,
                    expected_hash: None,
                    path: "people/alice.md".into(),
                    title: "Alice".into(),
                    icon: "user".into(),
                    body: String::new(),
                    sources: vec![],
                },
                MemoryPageChange {
                    id: None,
                    expected_hash: None,
                    path: "people/alice/preferences.md".into(),
                    title: "Preferences".into(),
                    icon: "sparkles".into(),
                    body: String::new(),
                    sources: vec![],
                },
            ],
            deletes: vec![],
        })
        .expect("create hierarchy");
    assert_eq!(
        memory
            .read_page("people/alice/preferences.md")
            .expect("deep page")
            .ancestors
            .into_iter()
            .map(|ancestor| ancestor.path)
            .collect::<Vec<_>>(),
        ["people.md", "people/alice.md"]
    );
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![],
            deletes: vec!["people.md".into()],
        })
        .is_err());

    let alice = memory.read_page("people/alice.md").expect("alice");
    memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: Some(alice.id.clone()),
                expected_hash: Some(alice.hash),
                path: "people/alicia.md".into(),
                title: "Alicia".into(),
                icon: "user".into(),
                body: String::new(),
                sources: vec![],
            }],
            deletes: vec!["people/alice/preferences.md".into()],
        })
        .expect("move page");
    assert!(memory.read_page("people/alice.md").is_err());
    assert_eq!(
        memory.read_page("people/alicia.md").expect("moved").id,
        alice.id
    );
}
