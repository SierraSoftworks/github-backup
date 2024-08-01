use std::sync::atomic::AtomicBool;

use tokio_stream::Stream;

use crate::{
    entities::{Credentials, HttpFile},
    errors::{self},
    helpers::{
        github::{GitHubRelease, GitHubRepo},
        GitHubClient,
    },
    policy::BackupPolicy,
    BackupSource,
};

pub const TAG_PRERELEASE: &str = "prerelease";
pub const TAG_SOURCE_CODE: &str = "source-code";

#[derive(Clone, Default)]
pub struct GitHubReleasesSource {
    client: GitHubClient,
}

impl GitHubReleasesSource {
    #[allow(dead_code)]
    pub fn with_client(client: GitHubClient) -> Self {
        Self { client }
    }
}

impl BackupSource<HttpFile> for GitHubReleasesSource {
    fn kind(&self) -> &str {
        "github/release"
    }

    fn validate(&self, policy: &BackupPolicy) -> Result<(), crate::Error> {
        let target = policy.from.as_str().trim_matches('/');
        match target {
          t if t.is_empty() => Err(errors::user(
              "The target field is required for GitHub repository backups.",
              "Please provide a target field in the policy using the format 'users/<username>' or 'orgs/<orgname>'.",
          )),

          t if t.chars().filter(|c| *c == '/').count() > 1 => Err(errors::user(
              &format!("The target field '{target}' contains too many segments."),
              "Please provide a target field in the policy using the format 'users/<username>' or 'orgs/<orgname>'.",
          )),

          t if !t.starts_with("users/") && !t.starts_with("orgs/") => Err(errors::user(
              &format!("The target field '{target}' does not include a valid user or org specifier."),
              "Please specify either 'users/<username>' or 'orgs/<orgname>' as your target.",
          )),

          _ => Ok(()),
      }
    }

    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<HttpFile, crate::Error>> + 'a {
        let url = format!(
            "{}/{}/repos",
            policy
                .properties
                .get("api_url")
                .as_deref()
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            &policy.from.trim_matches('/')
        );

        async_stream::stream! {
          for await repo in self.client.get_paginated::<GitHubRepo>(url, &policy.credentials, cancel) {
            if let Err(e) = repo {
              yield Err(e);
              continue;
            }

            let repo: GitHubRepo = repo.unwrap();

            if !repo.has_downloads {
              continue;
            }

            let releases_url = format!("{}/releases", repo.url);

            for await release in self.client.get_paginated::<GitHubRelease>(releases_url, &policy.credentials, cancel) {
              if let Err(e) = release {
                yield Err(e);
                continue;
              }

              let release: GitHubRelease = release.unwrap();

              if let Some(tarball_url) = &release.tarball_url {
                yield Ok(HttpFile::new(&repo.name, tarball_url)
                    .with_filename(format!("{}/{}/source.tar.gz", &repo.full_name, &release.tag_name))
                    .with_credentials(match &policy.credentials {
                      Credentials::Token(token) => Credentials::UsernamePassword {
                        username: token.clone(),
                        password: "".to_string(),
                      },
                      creds => creds.clone(),
                    })
                    .with_last_modified(release.published_at)
                    .with_optional_tag(if repo.size == 0 {
                        Some(crate::entities::git_repo::TAG_EMPTY)
                    } else {
                        None
                    })
                    .with_optional_tag(if repo.archived {
                        Some(crate::entities::git_repo::TAG_ARCHIVED)
                    } else {
                        None
                    })
                    .with_optional_tag(if repo.fork {
                        Some(crate::entities::git_repo::TAG_FORK)
                    } else {
                        None
                    })
                    .with_optional_tag(if repo.private {
                        Some(crate::entities::git_repo::TAG_PRIVATE)
                    } else {
                        Some(crate::entities::git_repo::TAG_PUBLIC)
                    })
                    .with_optional_tag(if release.prerelease {
                        Some(TAG_PRERELEASE)
                    } else {
                        None
                    })
                    .with_optional_tag(Some(TAG_SOURCE_CODE)));
              }

              for asset in release.assets {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  return;
                }

                if asset.state != "uploaded" {
                  continue;
                }

                let asset_url = format!("{}/releases/assets/{}", repo.url, asset.id);

                yield Ok(HttpFile::new(&repo.name, asset_url)
                    .with_filename(format!("{}/{}/{}", &repo.full_name, &release.tag_name, &asset.name))
                    .with_content_type(Some("application/octet-stream".to_string()))
                    .with_credentials(match &policy.credentials {
                      Credentials::Token(token) => Credentials::UsernamePassword {
                        username: token.clone(),
                        password: "".to_string(),
                      },
                      creds => creds.clone(),
                    })
                    .with_last_modified(Some(asset.updated_at))
                    .with_optional_tag(if repo.size == 0 {
                        Some(crate::entities::git_repo::TAG_EMPTY)
                    } else {
                        None
                    })
                    .with_optional_tag(if repo.archived {
                        Some(crate::entities::git_repo::TAG_ARCHIVED)
                    } else {
                        None
                    })
                    .with_optional_tag(if repo.fork {
                        Some(crate::entities::git_repo::TAG_FORK)
                    } else {
                        None
                    })
                    .with_optional_tag(if repo.private {
                        Some(crate::entities::git_repo::TAG_PRIVATE)
                    } else {
                        Some(crate::entities::git_repo::TAG_PUBLIC)
                    })
                    .with_optional_tag(if release.prerelease {
                        Some(TAG_PRERELEASE)
                    } else {
                        None
                    }));
              }
            }
          }
        }
    }
}
