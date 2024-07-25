use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

use gix::{credentials::helper::Action, progress::Discard, sec::identity::Account};
use tracing::{instrument, warn};

use crate::{config::Config, errors, BackupEntity, BackupTarget};

#[derive(Clone)]
pub struct FileSystemBackupTarget {
    target: Arc<PathBuf>,

    access_token: Arc<Option<String>>,
}

impl<T: BackupEntity + std::fmt::Debug> BackupTarget<T> for FileSystemBackupTarget {
    #[instrument(skip(self, cancel))]
    fn backup(&self, repo: &T, cancel: &AtomicBool) -> Result<String, errors::Error> {
        if !self.target.as_ref().exists() {
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
        }

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
            match self.fetch(repo, &target_path, cancel) {
                Ok(id) => Ok(id),
                Err(e) => {
                    warn!(error=%e, "Failed to fetch repository '{}', falling back to cloning it.", repo.full_name());
                    self.clone(repo, &target_path, cancel)
                }
            }
        } else {
            self.clone(repo, &target_path, cancel)
        }
    }
}

impl FileSystemBackupTarget {
    pub fn new<P: Into<PathBuf>>(target: P) -> Self {
        FileSystemBackupTarget {
            target: Arc::new(target.into()),
            access_token: Arc::new(None)
        }
    }

    pub fn with_access_token(mut self, token: String) -> Self {
        self.access_token = Arc::new(Some(token));
        self
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

        if let Some(token) = self.access_token.as_ref() {
            let token = token.clone();
            fetch = fetch.configure_connection(move |c| {
                let token = token.clone();
                c.set_credentials(move |a| match a {
                    Action::Get(ctx) => {
                        Ok(Some(gix::credentials::protocol::Outcome {
                            identity: Account {
                                username: token.clone(),
                                password: "".into(),
                            },
                            next: ctx.into()
                        }))
                    },
                    _ => Ok(None)
                });

                Ok(())
            });
        }

        let (repository, _outcome) = fetch.fetch_only(Discard, cancel).map_err(|e| errors::system_with_internal(
            &format!("Unable to clone remote repository '{}'", &repo.clone_url()),
            "Make sure that your internet connectivity is working correctly, and that your local git configuration is able to clone this repo.", 
            e))?;

        self.update_config(&repository, |c| {
            c.set_raw_value(&gix::config::tree::Core::BARE, "true").map_err(|e| errors::system_with_internal(
                &format!("Unable to set the 'core.bare' configuration option for repository '{}'", repo.full_name()),
                "Make sure that the git repository has been correctly initialized and run `git config core.bare true` to configure it correctly.",
                e))?;

            Ok(())
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

    fn update_config<U>(&self, repo: &gix::Repository, mut update: U) -> Result<(), errors::Error>
    where U: FnMut(&mut gix::config::File<'_>) -> Result<(), errors::Error>
    {
        let mut config = gix::config::File::from_path_no_includes(repo.path().join("config"), gix::config::Source::Local).map_err(|e| errors::system_with_internal(
            &format!("Unable to load git configuration for repository '{}'", repo.path().display()),
            "Make sure that the git repository has been correctly initialized.",
            e))?;
        
        update(&mut config)?;

        let mut file = std::fs::File::create(repo.path().join("config")).map_err(|e| errors::system_with_internal(
            &format!("Unable to write git configuration for repository '{}'", repo.path().display()),
            "Make sure that the git repository has been correctly initialized.",
            e))?;
        
        config.write_to(&mut file).map_err(|e| errors::system_with_internal(
            &format!("Unable to write git configuration for repository '{}'", repo.path().display()),
            "Make sure that the git repository has been correctly initialized.",
            e))
    }
}

impl From<&Config> for FileSystemBackupTarget {
    fn from(config: &Config) -> Self {
        let target = FileSystemBackupTarget::new(config.backup_path.clone());
        if let Some(token) = config.github.access_token.as_ref() {
            target.with_access_token(token.clone())
        } else {
            target
        }
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
