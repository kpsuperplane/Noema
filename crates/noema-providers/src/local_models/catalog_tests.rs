use super::*;
use crate::{LocalHardwareProfile, LocalModelBackend};

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REVISION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn bundled_catalog_loads_ranked_pinned_models() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");

    assert_eq!(catalog.models().len(), 1);
    assert_eq!(
        catalog
            .models()
            .iter()
            .map(|model| (model.id.as_str(), model.priority))
            .collect::<Vec<_>>(),
        vec![("gemma-4-e4b-it", 100)]
    );
    let model = &catalog.models()[0];
    assert_eq!(model.revision, "2714b5519c6c3516b1000e7c5e1eba998dfe1fe8");
    assert_eq!(model.builds.len(), 1);
    assert_eq!(model.builds[0].file, "gemma-4-E4B-it-Q4_K_M.gguf");
    assert_eq!(
        model.builds[0].sha256,
        "90ce98129eb3e8cc57e62433d500c97c624b1e3af1fcc85dd3b55ad7e0313e9f"
    );
    assert!(catalog.models().iter().all(|model| {
        model.revision.len() == 40 && model.builds.iter().all(|build| build.sha256.len() == 64)
    }));
}

#[test]
fn qualified_model_wins_only_at_its_memory_boundary() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let cases = [
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 12, None, true),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 15, None, true),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 16, None, true),
            Some("gemma-4-E4B-it-Q4_K_M.gguf"),
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 32, None, true),
            Some("gemma-4-E4B-it-Q4_K_M.gguf"),
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Cuda, 32, Some(24), false),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Vulkan, 32, Some(24), false),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Cpu, 32, None, false),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 11, None, true),
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
fn qualified_model_enforces_its_measured_ram_boundary() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let cases = [
        ("gemma-4-e4b-it", 15, None),
        ("gemma-4-e4b-it", 16, Some("gemma-4-E4B-it-Q4_K_M.gguf")),
    ];

    for (model_id, ram_gb, expected_file) in cases {
        let profiles = [LocalHardwareProfile::new(
            LocalModelBackend::Metal,
            ram_gb,
            None,
            true,
        )];
        let selection = catalog.select_build(model_id, &profiles);
        assert_eq!(
            selection.map(|selected| selected.build.file.as_str()),
            expected_file
        );
    }
}

#[test]
fn qualified_model_enforces_its_accelerator_memory_boundary() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let cases = [
        ("gemma-4-e4b-it", 5, None),
        ("gemma-4-e4b-it", 6, Some("gemma-4-E4B-it-Q4_K_M.gguf")),
    ];

    for (model_id, vram_gb, expected_file) in cases {
        let profiles = [LocalHardwareProfile::new(
            LocalModelBackend::Metal,
            32,
            Some(vram_gb),
            false,
        )];
        let selection = catalog.select_build(model_id, &profiles);
        assert_eq!(
            selection.map(|selected| selected.build.file.as_str()),
            expected_file
        );
    }
}

#[test]
fn backend_preference_skips_unqualified_backends() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let profiles = [
        LocalHardwareProfile::new(LocalModelBackend::Vulkan, 32, Some(24), false),
        LocalHardwareProfile::new(LocalModelBackend::Cpu, 32, None, false),
        LocalHardwareProfile::new(LocalModelBackend::Metal, 32, None, true),
    ];

    let recommendation = catalog
        .recommend_with_fallback(&profiles)
        .expect("the qualified Metal build should provide a fallback");

    assert_eq!(recommendation.hardware.backend, LocalModelBackend::Metal);
    assert_eq!(recommendation.build.file, "gemma-4-E4B-it-Q4_K_M.gguf");
}

#[test]
fn selecting_one_model_reuses_backend_fit_and_fallback() {
    let catalog = LocalModelCatalog::bundled().expect("bundled catalog must remain valid");
    let profiles = [
        LocalHardwareProfile::new(LocalModelBackend::Vulkan, 32, Some(24), false),
        LocalHardwareProfile::new(LocalModelBackend::Metal, 32, None, true),
    ];

    let selection = catalog
        .select_build("gemma-4-e4b-it", &profiles)
        .expect("the Metal artifact should fit this specific model");

    assert_eq!(selection.hardware.backend, LocalModelBackend::Metal);
    assert_eq!(selection.build.file, "gemma-4-E4B-it-Q4_K_M.gguf");
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
        .expect("Gemma E4B should fit");

    assert_eq!(
        recommendation.explanation(),
        "Recommended because Gemma 4 E4B IT fits your Metal backend and 16 GB of unified memory."
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
