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
            LocalHardwareProfile::new(LocalModelBackend::Metal, 32, Some(5), false),
            None,
        ),
        (
            LocalHardwareProfile::new(LocalModelBackend::Metal, 32, Some(6), false),
            Some("gemma-4-E4B-it-Q4_K_M.gguf"),
        ),
    ];

    for (hardware, expected_file) in cases {
        let selection = catalog.recommend_with_fallback(&[hardware]);
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
    let selection = catalog
        .select_build("gemma-4-e4b-it", &profiles)
        .expect("the same fallback must select a named model");
    assert_eq!(selection.hardware.backend, LocalModelBackend::Metal);
    assert!(catalog.select_build("missing", &profiles).is_none());
}

#[test]
fn recommendation_order_is_priority_then_catalog_order() {
    let source = [
        catalog_model("first", 10, "cpu", false),
        catalog_model("second", 20, "cpu", false),
        catalog_model("third", 20, "cpu", false),
    ]
    .join("\n");
    let catalog = LocalModelCatalog::parse(&source).expect("catalog should be valid");

    let recommendation = catalog
        .recommend_with_fallback(&[LocalHardwareProfile::new(
            LocalModelBackend::Cpu,
            8,
            None,
            false,
        )])
        .expect("one model should fit");

    assert_eq!(recommendation.model.id, "second");
}

#[test]
fn model_priority_wins_before_backend_preference() {
    let source = [
        catalog_model("accelerated-low-priority", 10, "vulkan", true),
        catalog_model("cpu-high-priority", 100, "cpu", false),
    ]
    .join("\n");
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
fn rejects_duplicate_ids_mutable_revisions_and_malformed_hashes() {
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
    for invalid in [
        valid_catalog().replace("priority = 1", "priority = 1\nenabled = true"),
        valid_catalog().replace("backends = [\"cpu\"]", "backends = [\"neural\"]"),
        format!("{}\n{}", valid_catalog(), valid_catalog()),
        valid_catalog().replace(REVISION, "main"),
        valid_catalog().replace(HASH, "not-a-hash"),
        missing,
        valid_catalog().replace("min_ram_gb = 1", "min_ram_gb = 0"),
    ] {
        assert!(LocalModelCatalog::parse(&invalid).is_err(), "{invalid}");
    }
}

fn valid_catalog() -> String {
    catalog_model("model", 1, "cpu", false)
}

fn catalog_model(id: &str, priority: u32, backend: &str, requires_vram: bool) -> String {
    let min_vram = if requires_vram { "min_vram_gb = 1" } else { "" };
    format!(
        r#"
[[models]]
id = "{id}"
name = "{id}"
license = "MIT"
priority = {priority}
repo = "owner/{id}"
revision = "{REVISION}"
[[models.builds]]
file = "{id}.gguf"
sha256 = "{HASH}"
download_gb = 1.0
backends = ["{backend}"]
min_ram_gb = 1
{min_vram}
"#
    )
}
