use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

use gix::progress::Discard;
use tracing::instrument;

use crate::{config::Config, errors, BackupEntity, BackupTarget};

#[derive(Clone)]
pub struct FileSystemBackupTarget {
    target: Arc<PathBuf>,
}

impl<T: BackupEntity + std::fmt::Debug> BackupTarget<T> for FileSystemBackupTarget {
    #[instrument(skip(self, cancel))]
    fn backup(&self, repo: &T, cancel: &AtomicBool) -> Result<String, errors::Error> {
        std::fs::create_dir_all(self.target.as_ref()).map_err(|e| {
            errors::user_with_internal(
                &format!(
                    "Unable to create backup directory '{}'",
                    &self.target.display()
                ),
                "Make sure that you have permission to create the directory.",
                e,
            )
        })?;

        let target_path = self.target.join(repo.backup_path());

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                errors::user_with_internal(
                    &format!("Unable to create backup directory '{}'", parent.display()),
                    "Make sure that you have permission to create the directory.",
                    e,
                )
            })?;
        }

        if target_path.exists() {
            self.fetch(repo, &target_path, cancel)
        } else {
            self.clone(repo, &target_path, cancel)
        }
    }
}

impl FileSystemBackupTarget {
    pub fn new<P: Into<PathBuf>>(target: P) -> Self {
        FileSystemBackupTarget {
            target: Arc::new(target.into()),
        }
    }

    fn clone<T: BackupEntity>(
        &self,
        repo: &T,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<String, errors::Error> {
        let mut fetch = gix::prepare_clone(repo.clone_url(), target).map_err(|e| errors::system_with_internal(
            &format!("Failed to clone the repository {}.", &repo.clone_url()),
            "Please make sure that the target directory is writable and that the repository is accessible.",
            e,
        ))?;

        let (mut checkout, _outcome) = fetch.fetch_then_checkout(Discard, cancel).map_err(|e| errors::system_with_internal(
            &format!("Unable to clone remote repository '{}'", &repo.clone_url()),
            "Make sure that your internet connectivity is working correctly, and that your local git configuration is able to clone this repo.", 
            e))?;

        let (repository, _outcome) = checkout.main_worktree(Discard, cancel).map_err(|e| {
            checkout.persist();

            errors::system_with_internal(
                &format!("Unable to checkout the repository '{}'", &repo.clone_url()),
                "Make sure that the repository is correctly cloned and that the target directory is writable.",
                e)
        })?;

        let head_id = repository.head_id().map_err(|e| errors::user_with_internal(
            &format!("The repository '{}' did not have a valid HEAD, which may indicate that there is something wrong with the source repository.", &repo.clone_url()),
            "Make sure that the remote repository is valid.",
            e))?;

        Ok(format!("{}", head_id.to_hex()))
    }

    fn fetch<T: BackupEntity>(
        &self,
        repo: &T,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<String, errors::Error> {
        let repository = gix::open(target).map_err(|e| {
            errors::user_with_internal(
                &format!(
                    "Failed to open the repository '{}' at '{}'",
                    repo.clone_url(),
                    &target.display()
                ),
                "Make sure that the target directory is a valid git repository.",
                e,
            )
        })?;

        let remote = repository.find_fetch_remote(None).map_err(|e| {
            errors::user_with_internal(
                &format!(
                    "Failed to find the remote '{}' in the repository '{}'",
                    repo.clone_url(),
                    &target.display()
                ),
                "Make sure that the repository is correctly configured and that the remote exists.",
                e,
            )
        })?;

        let _outcome = remote
            .connect(gix::remote::Direction::Fetch)
            .map_err(|e| {
                errors::user_with_internal(
                    &format!(
                        "Unable to establish connection to remote git repository '{}'",
                        repo.clone_url()
                    ),
                    "Make sure that the repository is available and correctly configured.",
                    e,
                )
            })?
            .prepare_fetch(Discard, Default::default())
            .map_err(|e| {
                errors::user_with_internal(
                    &format!(
                        "Unable to prepare fetch from remote git repository '{}'",
                        repo.clone_url()
                    ),
                    "Make sure that the repository is available and correctly configured.",
                    e,
                )
            })?
            .with_write_packed_refs_only(true)
            .receive(Discard, cancel)
            .map_err(|e| {
                errors::user_with_internal(
                    &format!(
                        "Unable to fetch from remote git repository '{}'",
                        &repo.clone_url()
                    ),
                    "Make sure that the repository is available and correctly configured.",
                    e,
                )
            })?;

        let head_id = repository.head_id().map_err(|e| errors::user_with_internal(
            &format!("The repository '{}' did not have a valid HEAD, which may indicate that there is something wrong with the source repository.", &repo.clone_url()),
            "Make sure that the remote repository is valid.",
            e))?;

        Ok(format!("{}", head_id.to_hex()))
    }
}

impl From<&Config> for FileSystemBackupTarget {
    fn from(config: &Config) -> Self {
        FileSystemBackupTarget::new(config.backup_path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "pure_tests"))]
    #[tokio::test]
    async fn test_backup() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let agent = FileSystemBackupTarget::new(temp_dir.path().to_path_buf());
        let cancel = AtomicBool::new(false);

        let repo = MockTarget;

        let id = agent
            .backup(&repo, &cancel)
            .expect("initial backup to succeed (clone)");
        assert!(
            temp_dir
                .path()
                .join(repo.backup_path())
                .join(".git")
                .exists(),
            "the repository should have been created"
        );

        let id2 = agent
            .backup(&repo, &cancel)
            .expect("subsequent backup to succeed (fetch)");
        assert_eq!(
            id, id2,
            "the repository should not have changed between backups"
        );
    }

    #[derive(Debug)]
    struct MockTarget;

    impl BackupEntity for MockTarget {
        fn backup_path(&self) -> PathBuf {
            PathBuf::from("SierraSoftworks/grey")
        }

        fn full_name(&self) -> &str {
            "SierraSoftworks/grey"
        }

        fn clone_url(&self) -> &str {
            "https://github.com/SierraSoftworks/grey.git"
        }
    }
}
