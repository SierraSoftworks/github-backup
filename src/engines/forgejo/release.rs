use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing_batteries::prelude::*;

use crate::{
    engines::{BackupState, summarize_states},
    entities::{Credentials, HttpFile, Release},
    errors::HumanizableError,
    helpers::forgejo::{
        CreateReleaseOptions, CreateReleaseResult, EditReleaseOptions, ForgejoClient,
    },
    target::RemoteTarget,
};

/// Artifacts which are generated for local backups but should not be uploaded
/// as release assets when replicating to Forgejo. The source tarball is
/// produced by GitHub on demand (and Forgejo generates its own), while the
/// release notes are replicated into the Forgejo release description instead.
const EXCLUDED_ASSETS: &[&str] = &["source.tar.gz", "RELEASE_NOTES.md"];

/// An engine which replicates releases (their notes and artifacts) to a Forgejo
/// instance, creating or updating the corresponding release as required.
#[derive(Clone, Default)]
pub struct ForgejoReleaseEngine {
    client: ForgejoClient,
    http: Arc<reqwest::Client>,
}

impl ForgejoReleaseEngine {
    #[tracing::instrument(skip(self, entity, target, cancel), fields(entity = %entity))]
    pub async fn backup(
        &self,
        entity: &Release,
        target: &RemoteTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let repo = forgejo_repo_name(&entity.full_name);

        let (release, release_state) = match self.ensure_release(target, &repo, entity).await? {
            Some(result) => result,
            None => return Ok(BackupState::Skipped),
        };

        let mut states = vec![release_state];

        for asset in &entity.assets {
            if cancel.load(Ordering::Relaxed) {
                return Ok(BackupState::Skipped);
            }

            let asset_name = asset_file_name(asset);
            if EXCLUDED_ASSETS.contains(&asset_name) {
                continue;
            }

            if release.has_asset(asset_name) {
                states.push(BackupState::Unchanged(Some(format!("asset {asset_name}"))));
                continue;
            }

            let data = self.download_asset(asset).await?;

            if cancel.load(Ordering::Relaxed) {
                return Ok(BackupState::Skipped);
            }

            self.client
                .upload_release_asset(target, &repo, release.id, asset_name, data)
                .await?;

            states.push(BackupState::New(Some(format!("asset {asset_name}"))));
        }

        Ok(summarize_states(&states))
    }

    /// Fetches the Forgejo release for the source release's tag, creating it
    /// (or updating its notes) as required so that it matches the source.
    ///
    /// Returns `None` when a release already exists but cannot be retrieved
    /// (for example a hidden draft synced onto a mirror), in which case the
    /// backup of this release is skipped rather than failing the whole policy.
    async fn ensure_release(
        &self,
        target: &RemoteTarget,
        repo: &str,
        entity: &Release,
    ) -> Result<Option<(crate::helpers::forgejo::Release, BackupState)>, crate::Error> {
        if let Some(release) = self
            .client
            .get_release_by_tag(target, repo, &entity.tag)
            .await?
        {
            let state = self
                .sync_release_notes(target, repo, &release, entity)
                .await?;
            return Ok(Some((release, state)));
        }

        trace!(
            "Release {} does not exist on Forgejo, creating it.",
            entity.tag
        );
        let options = CreateReleaseOptions::new(entity.tag.clone())
            .with_draft(entity.draft)
            .with_prerelease(entity.prerelease)
            .with_body(entity.body.clone());

        match self.client.create_release(target, repo, &options).await? {
            CreateReleaseResult::Created(release) => Ok(Some((
                release,
                BackupState::New(Some("release".to_string())),
            ))),
            CreateReleaseResult::AlreadyExists => {
                // Forgejo reports a 409 Conflict when a release already exists
                // for this tag, even though the lookup above returned nothing.
                // This happens for draft releases our token cannot surface, or
                // for tags synced onto a mirrored repository. Try the lookup
                // once more, and if the release still cannot be retrieved skip
                // it rather than failing the entire backup policy.
                match self
                    .client
                    .get_release_by_tag(target, repo, &entity.tag)
                    .await?
                {
                    Some(release) => {
                        let state = self
                            .sync_release_notes(target, repo, &release, entity)
                            .await?;
                        Ok(Some((release, state)))
                    }
                    None => {
                        warn!(
                            "A release for tag '{}' already exists on the Forgejo target but could not be retrieved; skipping it.",
                            entity.tag
                        );
                        Ok(None)
                    }
                }
            }
        }
    }

    /// Updates the Forgejo release's notes to match the source if they differ.
    async fn sync_release_notes(
        &self,
        target: &RemoteTarget,
        repo: &str,
        release: &crate::helpers::forgejo::Release,
        entity: &Release,
    ) -> Result<BackupState, crate::Error> {
        let desired = entity.body.as_deref().unwrap_or("");
        let existing = release.body.as_deref().unwrap_or("");

        if desired == existing {
            return Ok(BackupState::Unchanged(Some("release".to_string())));
        }

        trace!("Release notes for {} differ, updating them.", entity.tag);
        let options = EditReleaseOptions::new().with_body(entity.body.clone());
        self.client
            .update_release(target, repo, release.id, &options)
            .await?;

        Ok(BackupState::Updated(Some("release notes".to_string())))
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

/// Forgejo repository names cannot contain a `/`, so we use the final path
/// segment of the source repository's name as the Forgejo repository name.
fn forgejo_repo_name(full_name: &str) -> String {
    full_name
        .rsplit('/')
        .next()
        .unwrap_or(full_name)
        .to_string()
}

/// Release assets are named `owner/repo/tag/asset`; the file name uploaded to
/// Forgejo is the final path segment.
fn asset_file_name(asset: &HttpFile) -> &str {
    asset.name.rsplit('/').next().unwrap_or(&asset.name)
}
