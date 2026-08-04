use super::*;
use crate::{
    AdapterOperation, AdapterOperationBehavior, ArgumentLocation, ArgumentType,
    AuthenticationSchemeV4, CostClass, HttpMethod, OutputSchema, OutputType, PaginationPolicy,
    QuotaPolicy, ResponseContract, RetryPolicy,
};
use std::collections::BTreeMap;

fn manifest(definition_id: &str, adapter_id: &str) -> AdapterManifestV6 {
    AdapterManifestV6 {
        schema_version: 6,
        definition_id: definition_id.to_string(),
        adapter_id: adapter_id.to_string(),
        display_name: Some("Fixture".to_string()),
        definition_revision: "2026-07".to_string(),
        reviewed: true,
        origin: "https://api.example.test/".to_string(),
        authentication: AuthenticationSchemeV4::None,
        gates: vec![],
        quota: QuotaPolicy {
            cost_class: CostClass::Free,
            bucket: None,
            request_units: Some(1),
        },
        operations: vec![AdapterOperation {
            operation_id: "list".to_string(),
            description: "List items.".to_string(),
            source_description: None,
            method: HttpMethod::Get,
            path: "/v1/items".to_string(),
            fixed_headers: BTreeMap::new(),
            fixed_query: BTreeMap::new(),
            arguments: vec![crate::ArgumentDefinition {
                name: "limit".to_string(),
                description: "Maximum item count.".to_string(),
                source: crate::ArgumentSource::ModelInput,
                location: ArgumentLocation::Query,
                argument_type: ArgumentType::Integer,
                required: false,
                enum_values: vec![],
            }],
            json_body_template: None,
            behavior: AdapterOperationBehavior::model(true, true, false, true),
            retry: RetryPolicy::TransportSafeRead,
            pagination: PaginationPolicy::None,
            response: ResponseContract {
                accepted_content_types: vec!["application/json".to_string()],
                transform: None,
                output_schema: OutputSchema {
                    value_type: OutputType::Null,
                    properties: BTreeMap::new(),
                    required: Vec::new(),
                    additional_properties: None,
                    items: None,
                    max_bytes: None,
                    max_items: None,
                },
            },
            event: None,
            gates: vec![],
        }],
    }
}

#[test]
fn install_is_content_addressed_idempotent_and_scannable() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = AdapterDefinitionStore::new(paths.clone());
    let source = include_bytes!("../../tests/fixtures/github-openapi-source.json");
    let first = store
        .install(
            &manifest("definition:one", "one"),
            "https://github.com/github/rest-api-description",
            None,
            Some((source, "json")),
        )
        .expect("install");
    let second = store
        .install(
            &manifest("definition:one", "one"),
            "fixture://other-provenance",
            Some("2026-07-26T00:00:00Z"),
            Some((source, "json")),
        )
        .expect("idempotent install");
    assert_eq!(
        first.compiled.semantic_digest,
        second.compiled.semantic_digest
    );
    let scan = store.scan().expect("scan");
    assert_eq!(scan.definitions.len(), 1);
    assert!(scan.diagnostics.is_empty());
    assert_eq!(scan.projections()[0], first.projection);
    assert!(
        !fs::read_to_string(
            paths
                .adapter_definition_dir(first.compiled.semantic_digest.as_str())
                .expect("dir")
                .join(MANIFEST_FILE)
        )
        .expect("manifest")
        .contains("fixture://")
    );

    let stripe = store
        .install(
            &manifest("definition:two", "two"),
            "https://github.com/stripe/openapi",
            None,
            Some((
                include_bytes!("../../tests/fixtures/stripe-openapi-source.json"),
                "json",
            )),
        )
        .expect("second independent-company fixture");
    assert_eq!(
        first.compiled.operations[0].input_schema,
        stripe.compiled.operations[0].input_schema
    );
    assert_eq!(
        first.compiled.operations[0].behavior,
        stripe.compiled.operations[0].behavior
    );
}

#[test]
fn scan_blocks_tampered_symlinked_and_oversized_objects() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = AdapterDefinitionStore::new(paths.clone());
    let installed = store
        .install(
            &manifest("definition:one", "one"),
            "fixture://one",
            None,
            None,
        )
        .expect("install");
    let directory = paths
        .adapter_definition_dir(installed.compiled.semantic_digest.as_str())
        .expect("dir");
    fs::write(directory.join(MANIFEST_FILE), b"{}").expect("tamper");
    let scan = store.scan().expect("scan");
    assert!(scan.definitions.is_empty());
    assert_eq!(scan.diagnostics.len(), 1);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let other = home.path().join("other.json");
        fs::write(&other, b"{}").expect("other");
        fs::remove_file(directory.join(MANIFEST_FILE)).expect("remove");
        symlink(other, directory.join(MANIFEST_FILE)).expect("symlink");
        assert_eq!(
            store.scan().expect("scan").diagnostics[0].code,
            "object_file"
        );
    }
}

#[test]
fn exact_source_digest_detects_mutation_and_source_bounds() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = AdapterDefinitionStore::new(paths.clone());
    let source = b"official fixture";
    let installed = store
        .install(
            &manifest("definition:one", "one"),
            "fixture://one",
            None,
            Some((source, "yaml")),
        )
        .expect("install");
    let source_digest = installed
        .projection
        .source_digest
        .as_deref()
        .expect("source digest");
    fs::write(
        paths
            .adapter_source_path(source_digest, "yaml")
            .expect("source path"),
        b"mutated",
    )
    .expect("mutate");
    assert_eq!(
        store.scan().expect("scan").diagnostics[0].code,
        "source_digest"
    );
    assert!(matches!(
        store.install(
            &manifest("definition:two", "two"),
            "fixture://two",
            None,
            Some((&vec![0; MAX_SOURCE_BYTES + 1], "json"))
        ),
        Err(DefinitionStoreError::Integrity("source_oversized"))
    ));
}

#[test]
fn quarantine_conflict_preserves_different_active_and_quarantined_definitions() {
    let home = tempfile::tempdir().expect("home");
    let paths = NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = AdapterDefinitionStore::new(paths.clone());
    let definition = manifest("definition:one", "one");
    let installed = store
        .install(&definition, "fixture://first", None, None)
        .expect("first install");
    let digest = installed.compiled.semantic_digest.to_string();
    store.quarantine(&digest).expect("first quarantine");
    store
        .install(&definition, "fixture://different", None, None)
        .expect("second install");

    assert!(matches!(
        store.quarantine(&digest),
        Err(DefinitionStoreError::Integrity("quarantine_conflict"))
    ));
    let active = store.load(&digest).expect("active definition");
    let quarantined = store
        .read_stored_definition(
            &paths
                .adapter_quarantine_dir()
                .join("definitions")
                .join(&digest),
            &digest,
        )
        .expect("quarantined definition");
    assert_eq!(
        (
            active.provenance.source_reference.as_str(),
            quarantined.provenance.source_reference.as_str()
        ),
        ("fixture://different", "fixture://first")
    );
}
