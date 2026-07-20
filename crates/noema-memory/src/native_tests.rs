use super::*;
use tempfile::tempdir;

#[test]
fn native_memory_initializes_root_and_rebuilds_search_index() {
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
                body: "Alice likes tea".into(),
                sources: vec![],
            }],
            deletes: vec![],
        })
        .expect("publish");
    let child = &memory.read_root().expect("root").children[0];
    assert_eq!(child.path, "people.md");
    assert_eq!(child.excerpt, "Alice likes tea");
    assert_eq!(
        memory.search("Alice", 5).expect("search")[0].path,
        "people.md"
    );
    assert_eq!(
        memory.search("Alice \"", 5).expect("punctuated search")[0].path,
        "people.md"
    );
    assert_eq!(
        memory.read_page("people.md").expect("page").body,
        "Alice likes tea"
    );
    assert_eq!(
        memory
            .read_page("memory:human:people.md")
            .expect("page by stable id")
            .path,
        "people.md"
    );

    std::fs::write(
        directory.path().join("memory/human/root.md"),
        format!(
            "---\nschema: noema.memory.page/v1\nid: memory:human:root.md\nowner: human:local\nscope: human:local\ntitle: Kevin\nsources:\n---\n\n# Kevin\n\n{}",
            "Kevin has a durable preference for thoughtful technical systems. ".repeat(24)
        ),
    )
    .expect("write legacy root inventory");
    assert!(memory
        .publish(&MemoryChangeSet {
            upserts: vec![MemoryPageChange {
                id: None,
                expected_hash: None,
                path: "interests.md".into(),
                title: "Interests".into(),
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
            body: "Kevin has a durable preference for thoughtful technical systems. ".repeat(24),
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
                    body: String::new(),
                    sources: vec![],
                },
                MemoryPageChange {
                    id: None,
                    expected_hash: None,
                    path: "people/alice.md".into(),
                    title: "Alice".into(),
                    body: String::new(),
                    sources: vec![],
                },
                MemoryPageChange {
                    id: None,
                    expected_hash: None,
                    path: "people/alice/preferences.md".into(),
                    title: "Preferences".into(),
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
