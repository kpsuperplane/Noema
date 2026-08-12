//! Filesystem authority for reusable OAuth applications and account grants.

use crate::{
    AuthorizationGrantStatus, AuthorizationGrantV1, ExternalAccountV1,
    OauthApplicationCredentialV1, OauthApplicationV1, OauthGrantTokenV1, OauthProfileV1,
    SemanticDigest,
    oauth_authority_fs::{
        MAX_DESCRIPTOR_BYTES, MAX_SECRET_BYTES, STAGING_DIR, canonical_bytes, credential_file,
        install_directory, publish_generation, recover_staging,
    },
    oauth_authority_validation::{
        valid_hex, validate_account, validate_application, validate_grant, validate_profile,
    },
    private_fs::{
        PrivateFsError, create_private_dir, read_bounded_regular_file,
        require_directory_no_symlink, require_exact_entries, require_regular_directory,
    },
};
use noema_home::NoemaPaths;
use std::{collections::BTreeSet, fs, path::Path};
use thiserror::Error;

const PROFILE_FILE: &str = "profile.json";
const APPLICATION_FILE: &str = "application.json";
const ACCOUNT_FILE: &str = "account.json";
const GRANT_FILE: &str = "grant.json";
const CREDENTIALS_DIR: &str = "credentials";
const TOKENS_DIR: &str = "tokens";
const MAX_OBJECTS: usize = 1_024;

/// One reviewed profile and its exact content address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OauthProfileInstall {
    /// Exact content address and directory name.
    pub profile_digest: String,
    /// Reviewed public profile.
    pub profile: OauthProfileV1,
}

/// Complete filesystem-owned OAuth authority snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OauthAuthoritySnapshot {
    /// Valid reviewed profiles.
    pub profiles: Vec<OauthProfileInstall>,
    /// Valid public OAuth application descriptors.
    pub applications: Vec<OauthApplicationV1>,
    /// Valid external account descriptors.
    pub accounts: Vec<ExternalAccountV1>,
    /// Valid public authorization grant descriptors.
    pub grants: Vec<AuthorizationGrantV1>,
}

/// OAuth authority filesystem failure with no secret-bearing display fields.
#[derive(Debug, Error)]
pub enum OauthAuthorityStoreError {
    /// Filesystem operation failed.
    #[error("OAuth authority filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// A derived path component was invalid.
    #[error("OAuth authority path is invalid: {0}")]
    Path(#[from] noema_home::NoemaPathError),
    /// Canonical JSON could not be parsed or encoded.
    #[error("OAuth authority JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Filesystem content violated a stable authority invariant.
    #[error("OAuth authority invariant failed: {0}")]
    Integrity(&'static str),
}

impl From<PrivateFsError> for OauthAuthorityStoreError {
    fn from(error: PrivateFsError) -> Self {
        match error {
            PrivateFsError::Io(error) => Self::Io(error),
            PrivateFsError::Integrity(code) => Self::Integrity(code),
        }
    }
}

/// Canonical reusable OAuth authority store rooted in one `NOEMA_HOME`.
#[derive(Debug, Clone)]
pub struct OauthAuthorityStore {
    paths: NoemaPaths,
}

impl OauthAuthorityStore {
    /// Create a store handle. Directories are created lazily.
    #[must_use]
    pub const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    /// Create and validate all reusable OAuth authority roots.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when a path is unsafe or unavailable.
    pub fn prepare(&self) -> Result<(), OauthAuthorityStoreError> {
        require_directory_no_symlink(self.paths.root())?;
        create_private_dir(&self.paths.adapters_dir())?;
        for root in [
            self.paths.adapter_oauth_profiles_dir(),
            self.paths.adapter_oauth_applications_dir(),
            self.paths.adapter_external_accounts_dir(),
            self.paths.adapter_oauth_grants_dir(),
        ] {
            create_private_dir(&root)?;
            recover_staging(&root)?;
        }
        Ok(())
    }

    /// Install one immutable reviewed OAuth profile.
    ///
    /// Exact repeated imports are idempotent.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for invalid profile data or unsafe storage.
    pub fn install_profile(
        &self,
        profile: &OauthProfileV1,
    ) -> Result<OauthProfileInstall, OauthAuthorityStoreError> {
        validate_profile(profile)?;
        let bytes = canonical_bytes(profile)?;
        let digest = SemanticDigest::compute(&bytes).to_string();
        self.prepare()?;
        let target = self.paths.adapter_oauth_profile_dir(&digest)?;
        install_directory(
            &self.paths.adapter_oauth_profiles_dir(),
            &target,
            &[(PROFILE_FILE, bytes.as_slice())],
            &[],
            &[],
        )?;
        let installed = read_profile(&target, &digest)?;
        if installed.profile != *profile {
            return Err(OauthAuthorityStoreError::Integrity("profile_conflict"));
        }
        Ok(installed)
    }

    /// Install one OAuth application and its protected credential generation.
    ///
    /// Exact repeated imports are idempotent. A different credential requires
    /// the explicit replacement method.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for invalid links, conflicts, or unsafe storage.
    pub fn install_application(
        &self,
        application: &OauthApplicationV1,
        credential: &OauthApplicationCredentialV1,
    ) -> Result<OauthApplicationV1, OauthAuthorityStoreError> {
        self.prepare()?;
        let profile = self.load_profile(&application.profile_digest)?;
        validate_application(application, credential, &profile.profile)?;
        let descriptor = canonical_bytes(application)?;
        let secret = canonical_bytes(credential)?;
        let target = self
            .paths
            .adapter_oauth_application_dir(&application.application_id)?;
        install_directory(
            &self.paths.adapter_oauth_applications_dir(),
            &target,
            &[(APPLICATION_FILE, descriptor.as_slice())],
            &[CREDENTIALS_DIR],
            &[(
                CREDENTIALS_DIR,
                credential_file(&credential.generation_id),
                secret.as_slice(),
            )],
        )?;
        let (installed, installed_credential) =
            read_application(&target, &application.application_id)?;
        if installed != *application || installed_credential != *credential {
            return Err(OauthAuthorityStoreError::Integrity("application_conflict"));
        }
        Ok(installed)
    }

    /// Install one stable external account descriptor.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for invalid identity, conflicts, or unsafe storage.
    pub fn install_account(
        &self,
        account: &ExternalAccountV1,
    ) -> Result<ExternalAccountV1, OauthAuthorityStoreError> {
        self.prepare()?;
        self.load_profile(&account.profile_digest)?;
        validate_account(account)?;
        let bytes = canonical_bytes(account)?;
        let target = self
            .paths
            .adapter_external_account_dir(&account.account_id)?;
        install_directory(
            &self.paths.adapter_external_accounts_dir(),
            &target,
            &[(ACCOUNT_FILE, bytes.as_slice())],
            &[],
            &[],
        )?;
        let installed = read_account(&target, &account.account_id)?;
        if installed != *account {
            return Err(OauthAuthorityStoreError::Integrity("account_conflict"));
        }
        Ok(installed)
    }

    /// Install one grant and its optional current protected token generation.
    ///
    /// # Errors
    ///
    /// Returns a redacted error for invalid links, conflicts, or unsafe storage.
    pub fn install_grant(
        &self,
        grant: &AuthorizationGrantV1,
        token: Option<&OauthGrantTokenV1>,
    ) -> Result<AuthorizationGrantV1, OauthAuthorityStoreError> {
        self.prepare()?;
        let application = self.load_application(&grant.application_id)?;
        let profile = self.load_profile(&application.profile_digest)?;
        let account = grant
            .account_id
            .as_deref()
            .map(|id| self.load_account(id))
            .transpose()?;
        validate_grant(
            grant,
            token,
            &application,
            account.as_ref(),
            &profile.profile,
        )?;
        let descriptor = canonical_bytes(grant)?;
        let token_bytes = token.map(canonical_bytes).transpose()?;
        let nested = token.zip(token_bytes.as_ref()).map(|(token, bytes)| {
            (
                TOKENS_DIR,
                credential_file(&token.generation_id),
                bytes.as_slice(),
            )
        });
        let nested = nested.into_iter().collect::<Vec<_>>();
        let target = self.paths.adapter_oauth_grant_dir(&grant.grant_id)?;
        install_directory(
            &self.paths.adapter_oauth_grants_dir(),
            &target,
            &[(GRANT_FILE, descriptor.as_slice())],
            &[TOKENS_DIR],
            &nested,
        )?;
        let (installed, installed_token) = read_grant(&target, &grant.grant_id)?;
        if installed != *grant || installed_token.as_ref() != token {
            return Err(OauthAuthorityStoreError::Integrity("grant_conflict"));
        }
        Ok(installed)
    }

    /// Replace one application credential under an exact descriptor fence.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when authority changed or publication fails.
    pub fn replace_application_credential(
        &self,
        expected: &OauthApplicationV1,
        replacement: &OauthApplicationV1,
        credential: &OauthApplicationCredentialV1,
    ) -> Result<OauthApplicationV1, OauthAuthorityStoreError> {
        self.prepare()?;
        let target = self
            .paths
            .adapter_oauth_application_dir(&expected.application_id)?;
        let (current, current_credential) = read_application(&target, &expected.application_id)?;
        let profile = self.load_profile(&current.profile_digest)?;
        validate_application(replacement, credential, &profile.profile)?;
        if current != *expected
            || replacement.schema_version != current.schema_version
            || replacement.application_id != current.application_id
            || replacement.profile_digest != current.profile_digest
            || replacement.callback_mode != current.callback_mode
            || replacement.client_id != current.client_id
            || replacement.project_label != current.project_label
            || replacement.status != current.status
            || replacement.revision != current.revision.checked_add(1).unwrap_or(0)
            || replacement.credential_generation == current.credential_generation
        {
            return Err(OauthAuthorityStoreError::Integrity(
                "application_transition",
            ));
        }
        publish_generation(
            &target,
            APPLICATION_FILE,
            CREDENTIALS_DIR,
            &current_credential.generation_id,
            replacement,
            &credential.generation_id,
            credential,
        )?;
        Ok(read_application(&target, &replacement.application_id)?.0)
    }

    /// Promote a successful authorization or access expansion.
    ///
    /// Existing access stays unchanged unless this exact replacement publishes.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when authority changed or publication fails.
    pub fn promote_grant(
        &self,
        expected: &AuthorizationGrantV1,
        replacement: &AuthorizationGrantV1,
        token: &OauthGrantTokenV1,
    ) -> Result<AuthorizationGrantV1, OauthAuthorityStoreError> {
        self.replace_grant_generation(expected, replacement, token, false)
    }

    /// Rotate a grant token without changing delayed-work authority.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when authority changed or publication fails.
    pub fn refresh_grant(
        &self,
        expected: &AuthorizationGrantV1,
        replacement: &AuthorizationGrantV1,
        token: &OauthGrantTokenV1,
    ) -> Result<AuthorizationGrantV1, OauthAuthorityStoreError> {
        self.replace_grant_generation(expected, replacement, token, true)
    }

    /// Load one reviewed profile by its exact content address.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when the object is missing or invalid.
    pub fn load_profile(
        &self,
        digest: &str,
    ) -> Result<OauthProfileInstall, OauthAuthorityStoreError> {
        self.prepare()?;
        read_profile(&self.paths.adapter_oauth_profile_dir(digest)?, digest)
    }

    /// Load one public OAuth application descriptor.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when the object is missing or invalid.
    pub fn load_application(
        &self,
        id: &str,
    ) -> Result<OauthApplicationV1, OauthAuthorityStoreError> {
        self.prepare()?;
        let (application, _) =
            read_application(&self.paths.adapter_oauth_application_dir(id)?, id)?;
        Ok(application)
    }

    /// Load one external account descriptor.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when the object is missing or invalid.
    pub fn load_account(&self, id: &str) -> Result<ExternalAccountV1, OauthAuthorityStoreError> {
        self.prepare()?;
        read_account(&self.paths.adapter_external_account_dir(id)?, id)
    }

    /// Load one grant descriptor and current protected token for invocation.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when the object is missing or invalid.
    pub fn load_grant_authority(
        &self,
        id: &str,
    ) -> Result<(AuthorizationGrantV1, Option<OauthGrantTokenV1>), OauthAuthorityStoreError> {
        self.prepare()?;
        read_grant(&self.paths.adapter_oauth_grant_dir(id)?, id)
    }

    fn replace_grant_generation(
        &self,
        expected: &AuthorizationGrantV1,
        replacement: &AuthorizationGrantV1,
        token: &OauthGrantTokenV1,
        refresh: bool,
    ) -> Result<AuthorizationGrantV1, OauthAuthorityStoreError> {
        self.prepare()?;
        let target = self.paths.adapter_oauth_grant_dir(&expected.grant_id)?;
        let (current, current_token) = read_grant(&target, &expected.grant_id)?;
        let current_token =
            current_token.ok_or(OauthAuthorityStoreError::Integrity("grant_transition"))?;
        let application = self.load_application(&current.application_id)?;
        let profile = self.load_profile(&application.profile_digest)?;
        let account = current
            .account_id
            .as_deref()
            .map(|id| self.load_account(id))
            .transpose()?;
        validate_grant(
            replacement,
            Some(token),
            &application,
            account.as_ref(),
            &profile.profile,
        )?;
        let common = current == *expected
            && replacement.schema_version == current.schema_version
            && replacement.grant_id == current.grant_id
            && replacement.application_id == current.application_id
            && replacement.account_id == current.account_id
            && replacement.audience == current.audience
            && replacement.token_generation.as_deref() == Some(token.generation_id.as_str())
            && replacement.token_generation != current.token_generation
            && replacement.token_revision == current.token_revision.checked_add(1).unwrap_or(0)
            && replacement.status == AuthorizationGrantStatus::Active;
        let exact_revision = if refresh {
            replacement.desired_scopes == current.desired_scopes
                && replacement.granted_scopes == current.granted_scopes
                && replacement.authority_revision == current.authority_revision
        } else {
            replacement.authority_revision == current.authority_revision.checked_add(1).unwrap_or(0)
        };
        if !common || !exact_revision {
            return Err(OauthAuthorityStoreError::Integrity("grant_transition"));
        }
        publish_generation(
            &target,
            GRANT_FILE,
            TOKENS_DIR,
            &current_token.generation_id,
            replacement,
            &token.generation_id,
            token,
        )?;
        Ok(read_grant(&target, &replacement.grant_id)?.0)
    }

    /// Read one complete link-validated public authority snapshot.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when any root or object is invalid.
    pub fn snapshot(&self) -> Result<OauthAuthoritySnapshot, OauthAuthorityStoreError> {
        self.prepare()?;
        let profiles = scan_root(&self.paths.adapter_oauth_profiles_dir(), 64, read_profile)?;
        let applications = scan_root(
            &self.paths.adapter_oauth_applications_dir(),
            32,
            |path, name| read_application(path, name).map(|value| value.0),
        )?;
        let accounts = scan_root(
            &self.paths.adapter_external_accounts_dir(),
            32,
            read_account,
        )?;
        let grants = scan_root(&self.paths.adapter_oauth_grants_dir(), 32, |path, name| {
            read_grant(path, name).map(|value| value.0)
        })?;
        let profile_digests = profiles
            .iter()
            .map(|profile| profile.profile_digest.as_str())
            .collect::<BTreeSet<_>>();
        for application in &applications {
            if !profile_digests.contains(application.profile_digest.as_str()) {
                return Err(OauthAuthorityStoreError::Integrity("application_profile"));
            }
            let (_, credential) = read_application(
                &self
                    .paths
                    .adapter_oauth_application_dir(&application.application_id)?,
                &application.application_id,
            )?;
            let profile = profiles
                .iter()
                .find(|profile| profile.profile_digest == application.profile_digest)
                .ok_or(OauthAuthorityStoreError::Integrity("application_profile"))?;
            validate_application(application, &credential, &profile.profile)?;
        }
        for account in &accounts {
            if !profile_digests.contains(account.profile_digest.as_str()) {
                return Err(OauthAuthorityStoreError::Integrity("account_profile"));
            }
        }
        for grant in &grants {
            let application = applications
                .iter()
                .find(|application| application.application_id == grant.application_id)
                .ok_or(OauthAuthorityStoreError::Integrity("grant_application"))?;
            let profile = profiles
                .iter()
                .find(|profile| profile.profile_digest == application.profile_digest)
                .ok_or(OauthAuthorityStoreError::Integrity("grant_profile"))?;
            let account = grant
                .account_id
                .as_ref()
                .map(|id| {
                    accounts
                        .iter()
                        .find(|account| account.account_id == *id)
                        .ok_or(OauthAuthorityStoreError::Integrity("grant_account"))
                })
                .transpose()?;
            let token = read_grant(
                &self.paths.adapter_oauth_grant_dir(&grant.grant_id)?,
                &grant.grant_id,
            )?
            .1;
            validate_grant(
                grant,
                token.as_ref(),
                application,
                account,
                &profile.profile,
            )?;
        }
        Ok(OauthAuthoritySnapshot {
            profiles,
            applications,
            accounts,
            grants,
        })
    }
}

fn read_profile(
    path: &Path,
    digest: &str,
) -> Result<OauthProfileInstall, OauthAuthorityStoreError> {
    require_regular_directory(path)?;
    require_exact_entries(path, &[PROFILE_FILE])?;
    let bytes = read_bounded_regular_file(&path.join(PROFILE_FILE), MAX_DESCRIPTOR_BYTES)?;
    let profile: OauthProfileV1 = serde_json::from_slice(&bytes)?;
    validate_profile(&profile)?;
    if canonical_bytes(&profile)? != bytes || SemanticDigest::compute(&bytes).as_str() != digest {
        return Err(OauthAuthorityStoreError::Integrity("profile_identity"));
    }
    Ok(OauthProfileInstall {
        profile_digest: digest.to_string(),
        profile,
    })
}

fn read_application(
    path: &Path,
    id: &str,
) -> Result<(OauthApplicationV1, OauthApplicationCredentialV1), OauthAuthorityStoreError> {
    require_regular_directory(path)?;
    require_exact_entries(path, &[APPLICATION_FILE, CREDENTIALS_DIR])?;
    let bytes = read_bounded_regular_file(&path.join(APPLICATION_FILE), MAX_DESCRIPTOR_BYTES)?;
    let application: OauthApplicationV1 = serde_json::from_slice(&bytes)?;
    if application.application_id != id || canonical_bytes(&application)? != bytes {
        return Err(OauthAuthorityStoreError::Integrity("application_identity"));
    }
    let credentials = path.join(CREDENTIALS_DIR);
    require_regular_directory(&credentials)?;
    let credential_name = credential_file(&application.credential_generation);
    require_exact_entries(&credentials, &[&credential_name])?;
    let secret = read_bounded_regular_file(&credentials.join(&credential_name), MAX_SECRET_BYTES)?;
    let credential: OauthApplicationCredentialV1 = serde_json::from_slice(&secret)?;
    if credential.generation_id != application.credential_generation
        || canonical_bytes(&credential)? != secret
    {
        return Err(OauthAuthorityStoreError::Integrity(
            "application_credential",
        ));
    }
    Ok((application, credential))
}

fn read_account(path: &Path, id: &str) -> Result<ExternalAccountV1, OauthAuthorityStoreError> {
    require_regular_directory(path)?;
    require_exact_entries(path, &[ACCOUNT_FILE])?;
    let bytes = read_bounded_regular_file(&path.join(ACCOUNT_FILE), MAX_DESCRIPTOR_BYTES)?;
    let account: ExternalAccountV1 = serde_json::from_slice(&bytes)?;
    validate_account(&account)?;
    if account.account_id != id || canonical_bytes(&account)? != bytes {
        return Err(OauthAuthorityStoreError::Integrity("account_identity"));
    }
    Ok(account)
}

fn read_grant(
    path: &Path,
    id: &str,
) -> Result<(AuthorizationGrantV1, Option<OauthGrantTokenV1>), OauthAuthorityStoreError> {
    require_regular_directory(path)?;
    require_exact_entries(path, &[GRANT_FILE, TOKENS_DIR])?;
    let bytes = read_bounded_regular_file(&path.join(GRANT_FILE), MAX_DESCRIPTOR_BYTES)?;
    let grant: AuthorizationGrantV1 = serde_json::from_slice(&bytes)?;
    if grant.grant_id != id || canonical_bytes(&grant)? != bytes {
        return Err(OauthAuthorityStoreError::Integrity("grant_identity"));
    }
    let tokens = path.join(TOKENS_DIR);
    require_regular_directory(&tokens)?;
    let token = match grant.token_generation.as_deref() {
        Some(generation) => {
            let token_name = credential_file(generation);
            require_exact_entries(&tokens, &[&token_name])?;
            let bytes = read_bounded_regular_file(&tokens.join(token_name), MAX_SECRET_BYTES)?;
            let token: OauthGrantTokenV1 = serde_json::from_slice(&bytes)?;
            if token.generation_id != generation || canonical_bytes(&token)? != bytes {
                return Err(OauthAuthorityStoreError::Integrity("grant_token"));
            }
            Some(token)
        }
        None => {
            require_exact_entries(&tokens, &[])?;
            None
        }
    };
    Ok((grant, token))
}

fn scan_root<T>(
    root: &Path,
    name_length: usize,
    read: impl Fn(&Path, &str) -> Result<T, OauthAuthorityStoreError>,
) -> Result<Vec<T>, OauthAuthorityStoreError> {
    require_regular_directory(root)?;
    let mut entries = fs::read_dir(root)?.collect::<Result<Vec<_>, _>>()?;
    if entries.len() > MAX_OBJECTS + 1 {
        return Err(OauthAuthorityStoreError::Integrity("objects_oversized"));
    }
    entries.sort_by_key(fs::DirEntry::file_name);
    entries
        .into_iter()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name == STAGING_DIR {
                return None;
            }
            Some((entry.path(), name))
        })
        .map(|(path, name)| {
            if !valid_hex(&name, name_length) {
                return Err(OauthAuthorityStoreError::Integrity("object_name"));
            }
            read(&path, &name)
        })
        .collect()
}

#[cfg(test)]
mod tests;
