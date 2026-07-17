use std::path::{Path, PathBuf};

use noema_core::{
    HuggingFaceLocalModelImport, LocalModelInstaller, detect_local_hardware_profiles,
};
use noema_home::NoemaPaths;
use noema_store::{NoemaStore, StoreConfig};
use ring::digest::{Context, SHA256};
use tokio::{fs, io::AsyncReadExt};
use tokio_util::sync::CancellationToken;

use crate::manifest::ModelCandidate;

pub(crate) async fn resolve_candidate(
    candidate: &ModelCandidate,
    eval_home: &Path,
) -> Result<PathBuf, String> {
    let eval_paths =
        NoemaPaths::from_noema_home(eval_home.to_path_buf()).map_err(|error| error.to_string())?;
    let eval_blob = eval_paths
        .local_model_blob_path(&candidate.sha256)
        .map_err(|error| error.to_string())?;
    if verified_file(&eval_blob, &candidate.sha256).await? {
        return Ok(eval_blob);
    }

    if let Ok(user_paths) = NoemaPaths::from_process_env() {
        let user_blob = user_paths
            .local_model_blob_path(&candidate.sha256)
            .map_err(|error| error.to_string())?;
        if verified_file(&user_blob, &candidate.sha256).await? {
            return Ok(user_blob);
        }
    }

    println!(
        "downloading {} ({:.2} GB) from {}@{}",
        candidate.name,
        candidate.bytes as f64 / 1_000_000_000.0,
        candidate.repo,
        candidate.revision
    );
    let store = NoemaStore::open(&StoreConfig::new(eval_paths.sqlite_db_path()))
        .await
        .map_err(|error| error.to_string())?;
    let installer =
        LocalModelInstaller::new(store, eval_paths.clone()).map_err(|error| error.to_string())?;
    let backend = detect_local_hardware_profiles()
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "no local inference backend was detected".to_string())?
        .backend;
    let record = installer
        .import_hugging_face(
            HuggingFaceLocalModelImport {
                name: candidate.name.clone(),
                model_id: candidate.id.clone(),
                repo: candidate.repo.clone(),
                revision: candidate.revision.clone(),
                file: candidate.file.clone(),
                sha256: candidate.sha256.clone(),
                license: Some(candidate.license.clone()),
                backend,
            },
            CancellationToken::new(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let relative = record.blob_relative_path.ok_or_else(|| {
        format!(
            "download for {} completed without a blob path",
            candidate.id
        )
    })?;
    let path = eval_paths.root().join(relative);
    if !verified_file(&path, &candidate.sha256).await? {
        return Err(format!(
            "downloaded artifact for {} disappeared or failed verification",
            candidate.id
        ));
    }
    Ok(path)
}

async fn verified_file(path: &Path, expected: &str) -> Result<bool, String> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
    };
    if metadata.len() == 0 {
        return Ok(false);
    }
    let mut file = fs::File::open(path)
        .await
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut digest = Context::new(&SHA256);
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = digest
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(actual == expected)
}
