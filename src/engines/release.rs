use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use human_errors::ResultExt;

use crate::{
    BackupEntity,
    engines::{BackupState, ForgejoReleaseEngine, HttpFileEngine, summarize_states},
    entities::Release,
    target::{BackupTarget, RemoteTargetKind},
};

/// The file name used to store a release's notes when backing up to the local
/// filesystem.
const RELEASE_NOTES_FILE: &str = "RELEASE_NOTES.md";

/// A composite engine which backs up releases (their artifacts and notes)
/// either to the local filesystem or to a Forgejo instance, depending on the
/// configured target.
#[derive(Clone, Default)]
pub struct ReleaseEngine {
    http: HttpFileEngine,
    forgejo: ForgejoReleaseEngine,
}

impl ReleaseEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Backs up a release's assets and notes to the local filesystem.
    async fn backup_to_filesystem(
        &self,
        entity: &Release,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        let mut states = Vec::with_capacity(entity.assets.len() + 1);

        for asset in &entity.assets {
            if cancel.load(Ordering::Relaxed) {
                return Ok(BackupState::Skipped);
            }

            states.push(self.http.backup(asset, path, cancel).await?);
        }

        if let Some(state) = self.write_release_notes(entity, path).await? {
            states.push(state);
        }

        Ok(summarize_states(&states))
    }

    /// Writes the release notes to a `RELEASE_NOTES.md` file alongside the
    /// release's assets, returning the resulting backup state (or `None` when
    /// the release has no notes to back up).
    async fn write_release_notes(
        &self,
        entity: &Release,
        path: &Path,
    ) -> Result<Option<BackupState>, crate::Error> {
        let body = match &entity.body {
            Some(body) if !body.is_empty() => body,
            _ => return Ok(None),
        };

        let dir = path.join(entity.target_path());
        let notes_path = dir.join(RELEASE_NOTES_FILE);

        tokio::fs::create_dir_all(&dir).await.wrap_user_err(
            format!("Unable to create backup directory '{}'.", dir.display()),
            &["Make sure that you have permission to create the directory."],
        )?;

        let existing = tokio::fs::read_to_string(&notes_path).await.ok();

        let state = match existing {
            Some(existing) if existing == *body => {
                BackupState::Unchanged(Some("release notes".to_string()))
            }
            Some(_) => BackupState::Updated(Some("release notes".to_string())),
            None => BackupState::New(Some("release notes".to_string())),
        };

        if matches!(state, BackupState::Unchanged(_)) {
            return Ok(Some(state));
        }

        tokio::fs::write(&notes_path, body).await.wrap_user_err(
            format!(
                "Unable to write release notes to '{}'.",
                notes_path.display()
            ),
            &["Make sure that you have permission to write to this file/directory and try again."],
        )?;

        Ok(Some(state))
    }
}

#[async_trait::async_trait]
impl crate::engines::BackupEngine<Release> for ReleaseEngine {
    async fn backup(
        &self,
        entity: &Release,
        target: &BackupTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        match target {
            BackupTarget::FileSystem(path) => self.backup_to_filesystem(entity, path, cancel).await,
            BackupTarget::Remote(remote) => match remote.kind {
                RemoteTargetKind::ForgejoRelease => {
                    self.forgejo.backup(entity, remote, cancel).await
                }
                RemoteTargetKind::ForgejoRepo => Err(human_errors::user(
                    "You have configured a 'forgejo/repo' target for a release backup, which is not supported.",
                    &[
                        "Use a 'forgejo/release' target to back up release artifacts, or change the policy 'kind' to 'github/repo' to mirror repositories.",
                    ],
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use crate::engines::BackupEngine;

    use super::*;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    fn release_with_notes(body: Option<&str>) -> Release {
        Release::new("octocat/example/v1.0", "octocat/example", "v1.0")
            .with_body(body.map(|b| b.to_string()))
    }

    #[tokio::test]
    async fn writes_release_notes_to_filesystem() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");
        let engine = ReleaseEngine::new();
        let target = BackupTarget::FileSystem(temp_dir.path().to_path_buf());

        let entity = release_with_notes(Some("These are the release notes."));

        // First run creates the notes file.
        let state = engine.backup(&entity, &target, &CANCEL).await.unwrap();
        assert!(matches!(state, BackupState::New(_)));

        let notes_path = temp_dir
            .path()
            .join("octocat/example/v1.0")
            .join(RELEASE_NOTES_FILE);
        assert_eq!(
            std::fs::read_to_string(&notes_path).unwrap(),
            "These are the release notes."
        );

        // Re-running with the same notes leaves them unchanged.
        let state = engine.backup(&entity, &target, &CANCEL).await.unwrap();
        assert!(matches!(state, BackupState::Unchanged(_)));

        // Changing the notes results in an update.
        let updated = release_with_notes(Some("Revised release notes."));
        let state = engine.backup(&updated, &target, &CANCEL).await.unwrap();
        assert!(matches!(state, BackupState::Updated(_)));
        assert_eq!(
            std::fs::read_to_string(&notes_path).unwrap(),
            "Revised release notes."
        );
    }

    #[tokio::test]
    async fn skips_filesystem_backup_without_notes_or_assets() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");
        let engine = ReleaseEngine::new();
        let target = BackupTarget::FileSystem(temp_dir.path().to_path_buf());

        let entity = release_with_notes(None);

        let state = engine.backup(&entity, &target, &CANCEL).await.unwrap();
        assert!(matches!(state, BackupState::Skipped));

        assert!(
            !temp_dir
                .path()
                .join("octocat/example/v1.0")
                .join(RELEASE_NOTES_FILE)
                .exists()
        );
    }
}
