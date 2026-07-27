use super::*;
use crate::{
    AdapterOperation, AdapterOperationBehavior, ArgumentLocation, ArgumentType, AuthenticationMode,
    AuthenticationRequirement, CostClass, HttpMethod, PaginationPolicy, QuotaPolicy, RetryPolicy,
};
use std::collections::BTreeMap;

fn manifest(definition_id: &str, adapter_id: &str) -> AdapterManifestV3 {
    AdapterManifestV3 {
        schema_version: 3,
        definition_id: definition_id.to_string(),
        adapter_id: adapter_id.to_string(),
        display_name: Some("Fixture".to_string()),
        definition_revision: "2026-07".to_string(),
        reviewed: true,
        origin: "https://api.example.test/".to_string(),
        authentication: AuthenticationRequirement {
            mode: AuthenticationMode::None,
            scopes: vec![],
            client_setup_url: None,
            credential_import: None,
            oauth2: None,
        },
        gates: vec![],
        quota: QuotaPolicy {
            cost_class: CostClass::Free,
            bucket: None,
            request_units: Some(1),
        },
        operations: vec![AdapterOperation {
            operation_id: "list".to_string(),
            source_description: None,
            method: HttpMethod::Get,
            path: "/v1/items".to_string(),
            fixed_headers: BTreeMap::new(),
            arguments: vec![crate::ArgumentDefinition {
                name: "limit".to_string(),
                source: crate::ArgumentSource::ModelInput,
                location: ArgumentLocation::Query,
                argument_type: ArgumentType::Integer,
                required: false,
                enum_values: vec![],
            }],
            behavior: AdapterOperationBehavior::model(true, true, false, true),
            retry: RetryPolicy::TransportSafeRead,
            pagination: PaginationPolicy::None,
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
