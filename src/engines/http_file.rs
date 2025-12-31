use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

use human_errors::ResultExt;
use sha2::Digest;
use tokio::io::AsyncWriteExt;
use tracing_batteries::prelude::*;

use crate::{
    BackupEntity,
    entities::{Credentials, HttpFile},
    errors::HumanizableError,
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

    fn ensure_directory(&self, path: &Path) -> Result<(), human_errors::Error> {
        std::fs::create_dir_all(path).wrap_err_as_user(
            format!("Unable to create backup directory '{}'", path.display()),
            &["Make sure that you have permission to create the directory."],
        )
    }

    fn get_last_modified(&self, path: &Path) -> Option<chrono::DateTime<chrono::Utc>> {
        path.metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(chrono::DateTime::from)
    }

    async fn get_existing_sha256(&self, path: &Path) -> Option<String> {
        let sha_path = path.with_extension(
            format!(
                "{}.sha256",
                path.extension().unwrap_or_default().to_string_lossy()
            )
            .trim_start_matches('.'),
        );

        tokio::fs::read_to_string(sha_path)
            .await
            .map(|s| s.trim().to_owned())
            .ok()
    }
}

#[async_trait::async_trait]
impl BackupEngine<HttpFile> for HttpFileEngine {
    #[tracing::instrument(skip(self, entity, cancel, target), entity=%entity)]
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

        if let Some(origin_last_modified) = entity.last_modified
            && let Some(target_last_modified) = self.get_last_modified(&target_path)
                && target_last_modified >= origin_last_modified {
                    return Ok(BackupState::Unchanged(Some(format!(
                        "since {}",
                        target_last_modified.format("%Y-%m-%dT%H:%M:%S")
                    ))));
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

        let mut resp = req.send().await.map_err(|e| e.to_human_error())?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(human_errors::wrap_user(
                crate::errors::ResponseError::with_body(resp).await,
                format!(
                    "Got an HTTP {status} status code when trying to fetch '{}'.",
                    entity.url.as_str(),
                ),
                &[
                    "Make sure that you can access the URL and update your backup configuration if not.",
                ],
            ));
        }

        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let temp_path = target_path.with_extension(
            format!(
                "{}.tmp",
                target_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
            )
            .trim_start_matches('.'),
        );

        let mut file = tokio::fs::File::create(temp_path.as_path())
            .await
            .wrap_err_as_user(
                format!(
                    "Unable to create temporary backup file '{}'.",
                    temp_path.as_path().display()
                ),
                &["Make sure that you have permission to write to this file/directory and try again."],
            )?;

        let mut shasum = sha2::Sha256::new();

        while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_human_error())? {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                drop(file);
                tokio::fs::remove_file(&temp_path)
                    .await
                    .unwrap_or_else(|e| {
                        error!(
                            "Failed to remove temporary backup file '{}': {}",
                            temp_path.display(),
                            e
                        );
                    });
                return Ok(BackupState::Skipped);
            }

            match file.write_all(&chunk).await {
                Ok(()) => {
                    _ = shasum.update(chunk.as_ref());
                }
                Err(e) => {
                    drop(file);
                    tokio::fs::remove_file(&temp_path)
                        .await
                        .unwrap_or_else(|e| {
                            error!(
                                "Failed to remove temporary backup file '{}': {}",
                                temp_path.display(),
                                e
                            );
                        });
                    return Err(human_errors::wrap_user(
                        e,
                        format!(
                            "Failed to write to temporary backup file '{}'.",
                            temp_path.display()
                        ),
                        &[
                            "Make sure that you have permission to write to this file/directory and try again.",
                        ],
                    ));
                }
            }
        }

        drop(file);

        let shasum = shasum.finalize();
        if let Some(existing_sha256) = self.get_existing_sha256(&target_path).await
            && existing_sha256 == format!("{:x}", shasum) {
                tokio::fs::remove_file(&temp_path).await.map_err(|e| human_errors::wrap_user(
                    e,
              format!("Unable to remove temporary backup file '{}' after verifying that it is a duplicate of the existing file.", temp_path.display()),
              &["Make sure that you have write (and delete) permission on the backup directory and try again."],
              ))?;
                return Ok(BackupState::Unchanged(Some(format!(
                    "at sha256@{shasum:x}"
                ))));
            }

        let state = if target_path.exists() {
            tokio::fs::remove_file(&target_path).await.wrap_err_as_user(
              format!("Unable to remove original backup file '{}' prior to replacement with new file.", target_path.display()),
              &["Make sure that you have write (and delete) permission on the backup directory and try again."],
              )?;
            BackupState::Updated(
                entity
                    .last_modified
                    .map(|m| format!("at {}", m.format("%Y-%m-%dT%H:%M:%S")))
                    .or(Some(format!("at sha256:{shasum:x}"))),
            )
        } else {
            BackupState::New(
                entity
                    .last_modified
                    .map(|m| format!("at {}", m.format("%Y-%m-%dT%H:%M:%S")))
                    .or(Some(format!("at sha256:{shasum:x}"))),
            )
        };

        tokio::fs::rename(&temp_path, &target_path).await.map_err(|e| human_errors::wrap_user(
            e,
          format!("Unable to move temporary backup file '{}' to final location '{}'.", temp_path.display(), target_path.display()),
          &["Make sure that you have permission to write to this file/directory and try again."],
          ))?;

        tokio::fs::write(
            target_path.with_extension(format!(
                "{}.sha256",
                target_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
            )),
            format!("{:x}", shasum),
        )
        .await
        .wrap_err_as_user(
            format!(
                "Unable to write SHA-256 checksum file for backup file '{}'.",
                target_path.display()
            ),
            &["Make sure that you have permission to write to this file/directory and try again."],
        )?;

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_backup() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        // Create test data of 1024 bytes
        let test_data = vec![0u8; 1024];

        // Start a mock server
        let mock_server = MockServer::start().await;

        // Set up the mock endpoint
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_data.clone()))
            .mount(&mock_server)
            .await;

        let engine = HttpFileEngine::new();
        let cancel = AtomicBool::new(false);

        let entity = HttpFile {
            url: format!("{}/test-file", mock_server.uri()),
            name: "test.bin".to_string(),
            credentials: Credentials::None,
            metadata: Default::default(),
            last_modified: None,
            content_type: None,
        };

        // First backup should create a new file
        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert!(matches!(state, BackupState::New(Some(msg)) if msg.starts_with("at sha256:")));

        assert!(
            temp_dir.path().join(entity.target_path()).exists(),
            "the file should exist"
        );

        // Verify the file content and SHA-256 were stored correctly
        let file_path = temp_dir.path().join(entity.target_path());
        let content = std::fs::read(&file_path).expect("read file");
        assert_eq!(content.len(), 1024);

        let sha_path = file_path.with_extension("bin.sha256");
        assert!(sha_path.exists(), "SHA-256 checksum file should exist");

        // Second backup with same content should detect it's unchanged via SHA-256
        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert!(matches!(state, BackupState::Unchanged(Some(msg)) if msg.starts_with("at sha256")));
    }

    #[tokio::test]
    async fn test_backup_with_last_modified() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        // Create test data of 1024 bytes
        let test_data = vec![0u8; 1024];

        // Start a mock server
        let mock_server = MockServer::start().await;

        // Set up the mock endpoint
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(test_data.clone()))
            .mount(&mock_server)
            .await;

        let engine = HttpFileEngine::new();
        let cancel = AtomicBool::new(false);

        // Set last_modified to a time in the future to ensure first backup happens
        let last_modified = chrono::Utc::now() + chrono::Duration::days(1);

        let entity = HttpFile {
            url: format!("{}/test-file", mock_server.uri()),
            name: "test.bin".to_string(),
            credentials: Credentials::None,
            metadata: Default::default(),
            last_modified: Some(last_modified),
            content_type: None,
        };

        let state = engine
            .backup(&entity, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert_eq!(
            state,
            BackupState::New(Some(format!(
                "at {}",
                entity.last_modified.unwrap().format("%Y-%m-%dT%H:%M:%S")
            )))
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

        // For the second backup, use an older last_modified time so it short-circuits
        // based on the file's modification time being newer
        let entity_old = HttpFile {
            last_modified: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            ..entity
        };

        let state = engine
            .backup(&entity_old, temp_dir.path(), &cancel)
            .await
            .expect("backup to succeed");

        assert_eq!(
            state,
            BackupState::Unchanged(Some(format!(
                "since {}",
                backup_modified.format("%Y-%m-%dT%H:%M:%S")
            )))
        );
    }
}
