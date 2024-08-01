use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

use tokio::io::AsyncWriteExt;
use tracing::instrument;

use crate::{
    entities::{Credentials, HttpFile},
    errors, BackupEntity,
};

use super::{BackupEngine, BackupState};

#[derive(Clone)]
pub struct HttpFileEngine {
    client: Arc<reqwest::Client>,
}

impl HttpFileEngine {
    pub fn new() -> Self {
        Self {
            client: Arc::new(reqwest::Client::new()),
        }
    }

    fn ensure_directory(&self, path: &Path) -> Result<(), errors::Error> {
        std::fs::create_dir_all(path).map_err(|e| {
            errors::user_with_internal(
                &format!("Unable to create backup directory '{}'", path.display()),
                "Make sure that you have permission to create the directory.",
                e,
            )
        })
    }

    fn get_last_modified(&self, path: &Path) -> Option<chrono::DateTime<chrono::Utc>> {
        path.metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(|modified| chrono::DateTime::from(modified))
    }
}

#[async_trait::async_trait]
impl BackupEngine<HttpFile> for HttpFileEngine {
    #[instrument(skip(self, cancel, target))]
    async fn backup<P: AsRef<Path> + Send>(
        &self,
        entity: &HttpFile,
        target: P,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        let target_path = target.as_ref().join(entity.target_path());
        if let Some(parent) = target_path.parent() {
            self.ensure_directory(parent)?;
        }

        if let Some(origin_last_modified) = entity.last_modified {
            if let Some(target_last_modified) = self.get_last_modified(&target_path) {
                if target_last_modified >= origin_last_modified {
                    return Ok(BackupState::Unchanged(Some(format!(
                        "{}",
                        target_last_modified
                    ))));
                }
            }
        }

        let req = self
            .client
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

        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let mut resp = req.send().await?;

        if !resp.status().is_success() {
            return Err(errors::user_with_internal(
                &format!(
                    "Got an HTTP {} status code when trying to fetch '{}'.",
                    resp.status(),
                    entity.url.as_str(),
                ),
                "Make sure that you can access the URL and update your backup configuration if not.",
                errors::ResponseError::with_body(resp).await
            ));
        }

        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let temp_path = target_path.with_extension("backup.inprogress");

        let mut file = tokio::fs::File::create(temp_path.as_path())
            .await
            .map_err(|e| {
                errors::user_with_internal(
                &format!(
                    "Unable to create temporary backup file '{}'.",
                    temp_path.as_path().display()
                ),
                "Make sure that you have permission to write to this file/directory and try again.",
                e,
            )
            })?;

        let mut size = 0;
        while let Some(chunk) = resp.chunk().await? {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                drop(file);
                tokio::fs::remove_file(&temp_path)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!(
                            "Failed to remove temporary backup file '{}': {}",
                            temp_path.display(),
                            e
                        );
                    });
                return Ok(BackupState::Skipped);
            }

            match file.write_all(&chunk).await {
                Ok(()) => {
                    size += chunk.len();
                }
                Err(e) => {
                    drop(file);
                    tokio::fs::remove_file(&temp_path)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::error!(
                                "Failed to remove temporary backup file '{}': {}",
                                temp_path.display(),
                                e
                            );
                        });
                    return Err(errors::user_with_internal(
                      &format!("Failed to write to temporary backup file '{}'.", temp_path.display()),
                      "Make sure that you have permission to write to this file/directory and try again.",
                      e
                    ));
                }
            }
        }

        drop(file);

        let state = if target_path.exists() {
            tokio::fs::remove_file(&target_path).await.map_err(|e| errors::user_with_internal(
              &format!("Unable to remove original backup file '{}' prior to replacement with new file.", target_path.display()),
              "Make sure that you have write (and delete) permission on the backup directory and try again.",
              e))?;
            BackupState::Updated(
                entity
                    .last_modified
                    .map(|m| format!("{}", m))
                    .or(Some(format!("{size} bytes"))),
            )
        } else {
            BackupState::New(
                entity
                    .last_modified
                    .map(|m| format!("{}", m))
                    .or(Some(format!("{size} bytes"))),
            )
        };

        tokio::fs::rename(&temp_path, &target_path).await.map_err(|e| errors::user_with_internal(
          &format!("Unable to move temporary backup file '{}' to final location '{}'.", temp_path.display(), target_path.display()),
          "Make sure that you have permission to write to this file/directory and try again.",
          e))?;

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "pure_tests"))]
    #[tokio::test]
    async fn test_backup() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let engine = HttpFileEngine::new();
        let cancel = AtomicBool::new(false);

        let entity = HttpFile {
            url: "https://httpbin.org/bytes/1024".to_string(),
            name: "test.bin".to_string(),
            credentials: Credentials::None,
            tags: Default::default(),
            last_modified: None,
        };

        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert_eq!(state, BackupState::New(Some("1024 bytes".to_string())));

        assert!(
            temp_dir.path().join(entity.target_path()).exists(),
            "the file should exist"
        );

        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert_eq!(state, BackupState::Updated(Some("1024 bytes".to_string())));
    }

    #[cfg(not(feature = "pure_tests"))]
    #[tokio::test]
    async fn test_backup_with_last_modified() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let engine = HttpFileEngine::new();
        let cancel = AtomicBool::new(false);

        let entity = HttpFile {
            url: "https://httpbin.org/bytes/1024".to_string(),
            name: "test.bin".to_string(),
            credentials: Credentials::None,
            tags: Default::default(),
            last_modified: Some(chrono::Utc::now()),
        };

        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert_eq!(
            state,
            BackupState::New(Some(format!("{}", entity.last_modified.unwrap())))
        );

        assert!(
            temp_dir.path().join(entity.target_path()).exists(),
            "the file should exist"
        );

        let backup_modified: chrono::DateTime<chrono::Utc> = temp_dir
            .path()
            .join(entity.target_path())
            .metadata()
            .expect("metadata")
            .modified()
            .expect("modified")
            .into();

        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert_eq!(
            state,
            BackupState::Unchanged(Some(format!("{}", backup_modified)))
        );
    }
}
