use super::*;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REVISION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn bundled_catalog_loads_and_contains_pinned_bonsai_builds() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");

    assert_eq!(catalog.models().len(), 1);
    let bonsai = &catalog.models()[0];
    assert_eq!(bonsai.id, "ternary-bonsai-8b");
    assert_eq!(bonsai.priority, 100);
    assert_eq!(bonsai.builds.len(), 2);
    assert_eq!(bonsai.revision.len(), 40);
    assert!(bonsai.builds.iter().all(|build| build.sha256.len() == 64));
}

#[test]
fn bonsai_fits_each_boundary_and_wins_only_when_it_fits() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let cases = [
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 12, None, true),
            Some("Bonsai-8B-Q2_KT.gguf"),
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Cuda, 12, Some(6), false),
            Some("Bonsai-8B-Q2_KT.gguf"),
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Vulkan, 12, Some(6), false),
            Some("Bonsai-8B-Q2_KT.gguf"),
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Cpu, 12, None, false),
            Some("Bonsai-8B-TQ2_0.gguf"),
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 11, None, true),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Cuda, 12, Some(5), false),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Cpu, 11, None, false),
            None,
        ),
    ];

    for (hardware, expected_file) in cases {
        let recommendation = catalog.recommend(hardware);
        assert_eq!(
            recommendation.map(|selected| selected.build.file.as_str()),
            expected_file
        );
    }
}

#[test]
fn backend_preference_falls_back_to_cpu() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let profiles = [
        LocalHardwareProfile::new(LocalModelBackend::Vulkan, 12, Some(5), false),
        LocalHardwareProfile::new(LocalModelBackend::Cpu, 12, None, false),
    ];

    let recommendation = catalog
        .recommend_with_fallback(&profiles)
        .expect("CPU build should provide a fallback");

    assert_eq!(recommendation.hardware.backend, LocalModelBackend::Cpu);
    assert_eq!(recommendation.build.file, "Bonsai-8B-TQ2_0.gguf");
}

#[test]
fn selecting_one_model_reuses_backend_fit_and_fallback() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let profiles = [
        LocalHardwareProfile::new(LocalModelBackend::Vulkan, 12, Some(5), false),
        LocalHardwareProfile::new(LocalModelBackend::Cpu, 12, None, false),
    ];

    let selection = catalog
        .select_build("ternary-bonsai-8b", &profiles)
        .expect("the CPU artifact should fit this specific model");

    assert_eq!(selection.hardware.backend, LocalModelBackend::Cpu);
    assert_eq!(selection.build.file, "Bonsai-8B-TQ2_0.gguf");
    assert!(catalog.select_build("missing", &profiles).is_none());
}

#[test]
fn recommendation_order_is_priority_then_catalog_order() {
    let source = format!(
        r#"
[[models]]
id = "first"
name = "First"
license = "MIT"
priority = 10
repo = "owner/first"
revision = "{REVISION}"
[[models.builds]]
file = "first.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["cpu"]
min_ram_gb = 1

[[models]]
id = "second"
name = "Second"
license = "MIT"
priority = 20
repo = "owner/second"
revision = "{REVISION}"
[[models.builds]]
file = "second.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["cpu"]
min_ram_gb = 1

[[models]]
id = "third"
name = "Third"
license = "MIT"
priority = 20
repo = "owner/third"
revision = "{REVISION}"
[[models.builds]]
file = "third.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["cpu"]
min_ram_gb = 1
"#
    );
    let catalog = LocalModelCatalog::parse(&source).expect("catalog should be valid");

    let recommendation = catalog
        .recommend(LocalHardwareProfile::new(
            LocalModelBackend::Cpu,
            8,
            None,
            false,
        ))
        .expect("one model should fit");

    assert_eq!(recommendation.model.id, "second");
}

#[test]
fn model_priority_wins_before_backend_preference() {
    let source = format!(
        r#"
[[models]]
id = "accelerated-low-priority"
name = "Accelerated low priority"
license = "MIT"
priority = 10
repo = "owner/accelerated"
revision = "{REVISION}"
[[models.builds]]
file = "accelerated.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["vulkan"]
min_ram_gb = 1
min_vram_gb = 1

[[models]]
id = "cpu-high-priority"
name = "CPU high priority"
license = "MIT"
priority = 100
repo = "owner/cpu"
revision = "{REVISION}"
[[models.builds]]
file = "cpu.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["cpu"]
min_ram_gb = 1
"#
    );
    let catalog = LocalModelCatalog::parse(&source).expect("catalog");
    let profiles = [
        LocalHardwareProfile::new(LocalModelBackend::Vulkan, 8, Some(8), false),
        LocalHardwareProfile::new(LocalModelBackend::Cpu, 8, None, false),
    ];

    let recommendation = catalog
        .recommend_with_fallback(&profiles)
        .expect("recommendation");

    assert_eq!(recommendation.model.id, "cpu-high-priority");
    assert_eq!(recommendation.hardware.backend, LocalModelBackend::Cpu);
}

#[test]
fn recommendation_copy_comes_from_matched_data() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let recommendation = catalog
        .recommend(LocalHardwareProfile::new(
            LocalModelBackend::Metal,
            16,
            None,
            true,
        ))
        .expect("Bonsai should fit");

    assert_eq!(
        recommendation.explanation(),
        "Recommended because Ternary Bonsai 8B fits your Metal backend and 16 GB of unified memory."
    );
}

#[test]
fn rejects_unknown_fields_and_invalid_backends() {
    let unknown = valid_catalog().replace("priority = 1", "priority = 1\nenabled = true");
    let backend = valid_catalog().replace("backends = [\"cpu\"]", "backends = [\"neural\"]");

    assert!(matches!(
        LocalModelCatalog::parse(&unknown),
        Err(LocalModelCatalogError::Toml(_))
    ));
    assert!(matches!(
        LocalModelCatalog::parse(&backend),
        Err(LocalModelCatalogError::Toml(_))
    ));
}

#[test]
fn rejects_duplicate_ids_mutable_revisions_and_malformed_hashes() {
    let duplicate = format!("{}\n{}", valid_catalog(), valid_catalog());
    let mutable = valid_catalog().replace(REVISION, "main");
    let malformed_hash = valid_catalog().replace(HASH, "not-a-hash");

    assert!(matches!(
        LocalModelCatalog::parse(&duplicate),
        Err(LocalModelCatalogError::DuplicateModelId(_))
    ));
    assert!(matches!(
        LocalModelCatalog::parse(&mutable),
        Err(LocalModelCatalogError::InvalidModelField {
            field: "revision",
            ..
        })
    ));
    assert!(matches!(
        LocalModelCatalog::parse(&malformed_hash),
        Err(LocalModelCatalogError::InvalidBuildField {
            field: "sha256",
            ..
        })
    ));
}

#[test]
fn rejects_missing_builds_and_impossible_memory() {
    let missing = format!(
        r#"
[[models]]
id = "model"
name = "Model"
license = "MIT"
priority = 1
repo = "owner/model"
revision = "{REVISION}"
"#
    );
    let impossible = valid_catalog().replace("min_ram_gb = 1", "min_ram_gb = 0");

    assert!(matches!(
        LocalModelCatalog::parse(&missing),
        Err(LocalModelCatalogError::MissingBuilds(_))
    ));
    assert!(matches!(
        LocalModelCatalog::parse(&impossible),
        Err(LocalModelCatalogError::ImpossibleMemory { .. })
    ));
}

fn valid_catalog() -> String {
    format!(
        r#"
[[models]]
id = "model"
name = "Model"
license = "MIT"
priority = 1
repo = "owner/model"
revision = "{REVISION}"
[[models.builds]]
file = "model.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["cpu"]
min_ram_gb = 1
"#
    )
}
