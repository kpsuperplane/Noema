use std::{
    env,
    path::{Path, PathBuf},
};

use noema_providers::{
    MaterializeVerifiedEvalModelRequest, VerifiedEvalModelSource, materialize_verified_eval_model,
};
use ring::digest::{Context, SHA256};
use tokio::{fs, io::AsyncReadExt};
use tokio_util::sync::CancellationToken;

use crate::manifest::ModelCandidate;

pub(crate) async fn resolve_candidate(
    candidate: &ModelCandidate,
    eval_home: &Path,
) -> Result<PathBuf, String> {
    let eval_cache = eval_home.join("models/blobs");
    let eval_blob = eval_cache.join(format!("{}.gguf", candidate.sha256));
    if verified_file(&eval_blob, &candidate.sha256).await? {
        return Ok(eval_blob);
    }

    if let Some(user_root) = user_noema_root() {
        let user_blob = user_root
            .join("models/blobs")
            .join(format!("{}.gguf", candidate.sha256));
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
    let path = materialize_verified_eval_model(
        MaterializeVerifiedEvalModelRequest {
            source: VerifiedEvalModelSource {
                repo: candidate.repo.clone(),
                revision: candidate.revision.clone(),
                file: candidate.file.clone(),
            },
            expected_bytes: candidate.bytes,
            sha256: candidate.sha256.clone(),
            cache_root: eval_cache,
        },
        CancellationToken::new(),
    )
    .await
    .map_err(|error| error.to_string())?;
    if !verified_file(&path, &candidate.sha256).await? {
        return Err(format!(
            "downloaded artifact for {} disappeared or failed verification",
            candidate.id
        ));
    }
    Ok(path)
}

fn user_noema_root() -> Option<PathBuf> {
    match env::var_os("NOEMA_HOME") {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        Some(_) => None,
        None => env::var_os("HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .map(|home| home.join(".noema")),
    }
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
