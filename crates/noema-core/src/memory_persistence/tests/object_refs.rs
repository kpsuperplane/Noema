use super::*;

#[test]
fn parses_and_formats_known_object_refs() {
    let object_ref = ObjectRef::new(ObjectType::ConversationItem, "item_123").expect("object ref");

    assert_eq!(object_ref.object_type.as_str(), "conversation_item");
    assert_eq!(object_ref.object_id, "item_123");
    assert_eq!(object_ref.to_string(), "conversation_item:item_123");
}

#[test]
fn rejects_unknown_object_type() {
    let error = ObjectType::parse("scope").expect_err("unknown object type");

    assert!(matches!(
        error,
        MemoryPersistenceError::InvalidObjectType { value } if value == "scope"
    ));
}

#[test]
fn validates_object_ref_targets() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let mut repo = SqliteMemoryRepository::open(&paths).expect("repo");

    repo.ensure_default_actors().expect("default actors");
    repo.validate_object_ref(&ObjectRef::human("human:local"))
        .expect("local human exists");

    let error = repo
        .validate_object_ref(
            &ObjectRef::new(ObjectType::Conversation, "missing").expect("missing ref"),
        )
        .expect_err("missing conversation");
    assert!(matches!(
        error,
        MemoryPersistenceError::ObjectRefNotFound { object_type, object_id }
          if object_type == "conversation" && object_id == "missing"
    ));
}
