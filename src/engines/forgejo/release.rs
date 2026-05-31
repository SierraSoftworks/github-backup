use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing_batteries::prelude::*;

use crate::{
    BackupEntity, FilterValue, Filterable,
    engines::BackupState,
    entities::{Credentials, HttpFile},
    errors::HumanizableError,
    helpers::forgejo::{CreateReleaseOptions, ForgejoClient},
    target::RemoteTarget,
};

/// An engine which uploads release artifacts to a Forgejo instance, creating
/// the corresponding release if it does not yet exist.
#[derive(Clone, Default)]
pub struct ForgejoReleaseEngine {
    client: ForgejoClient,
    http: Arc<reqwest::Client>,
}

impl ForgejoReleaseEngine {
    #[tracing::instrument(skip(self, entity, target, cancel), fields(entity = %entity))]
    pub async fn backup(
        &self,
        entity: &HttpFile,
        target: &RemoteTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let (repo, tag, asset_name) = parse_release_path(entity)?;

        let release = match self.client.get_release_by_tag(target, &repo, &tag).await? {
            Some(release) => release,
            None => {
                trace!("Release {tag} does not exist on Forgejo, creating it.");
                let draft = matches!(entity.get("release.draft"), FilterValue::Bool(true));
                let prerelease =
                    matches!(entity.get("release.prerelease"), FilterValue::Bool(true));
                let options = CreateReleaseOptions::new(tag.clone())
                    .with_draft(draft)
                    .with_prerelease(prerelease);
                self.client.create_release(target, &repo, &options).await?
            }
        };

        if release.has_asset(&asset_name) {
            return Ok(BackupState::Unchanged(Some(format!("asset {asset_name}"))));
        }

        if cancel.load(Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let data = self.download_asset(entity).await?;

        if cancel.load(Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        self.client
            .upload_release_asset(target, &repo, release.id, &asset_name, data)
            .await?;

        Ok(BackupState::New(Some(format!("asset {asset_name}"))))
    }

    async fn download_asset(&self, entity: &HttpFile) -> Result<Vec<u8>, crate::Error> {
        let req = self
            .http
            .get(entity.url.as_str())
            .header("User-Agent", "SierraSoftworks/github-backup");

        let req = if let Some(content_type) = &entity.content_type {
            req.header("Accept", content_type)
        } else {
            req
        };

        let req = match &entity.credentials {
            Credentials::None => req,
            Credentials::Token(token) => req.bearer_auth(token),
            Credentials::UsernamePassword { username, password } => {
                req.basic_auth(username, Some(password))
            }
        };

        let resp = req.send().await.map_err(|e| e.to_human_error())?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(human_errors::wrap_user(
                crate::errors::ResponseError::with_body(resp).await,
                format!(
                    "Got an HTTP {status} status code when trying to download '{}'.",
                    entity.url.as_str(),
                ),
                &[
                    "Make sure that you can access the URL and update your backup configuration if not.",
                ],
            ));
        }

        let bytes = resp.bytes().await.map_err(|e| e.to_human_error())?;
        Ok(bytes.to_vec())
    }
}

/// Release artifacts are named `owner/repo/tag/asset`; we extract the repo,
/// tag, and asset name so we can address the corresponding Forgejo release.
fn parse_release_path(entity: &HttpFile) -> Result<(String, String, String), crate::Error> {
    let parts: Vec<&str> = entity.name().splitn(4, '/').collect();
    if parts.len() != 4 {
        return Err(human_errors::user(
            format!(
                "The release artifact '{}' did not have the expected 'owner/repo/tag/asset' structure.",
                entity.name()
            ),
            &["This is likely a bug in github-backup, please report it to us on GitHub."],
        ));
    }

    Ok((
        parts[1].to_string(),
        parts[2].to_string(),
        parts[3].to_string(),
    ))
}
