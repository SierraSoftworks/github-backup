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
          "" => Err(errors::user(
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
        span: tracing::Span,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<HttpFile, crate::Error>> + 'a {
        let url = format!(
            "{}/{}/repos",
            policy
                .properties
                .get("api_url")
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            &policy.from.trim_matches('/')
        );

        async_stream::stream! {
          let _span = span.entered();
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
                yield Ok(HttpFile::new(format!("{}/{}/source.tar.gz", &repo.full_name, &release.tag_name), tarball_url)
                    .with_metadata_source(&repo)
                    .with_metadata_source(&release)
                    .with_metadata("asset.source-code", true)
                    .with_credentials(match &policy.credentials {
                      Credentials::Token(token) => Credentials::UsernamePassword {
                        username: token.clone(),
                        password: "".to_string(),
                      },
                      creds => creds.clone(),
                    })
                    .with_last_modified(release.published_at));
              }

              for asset in release.assets.iter() {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  return;
                }

                if asset.state != "uploaded" {
                  continue;
                }

                let asset_url = format!("{}/releases/assets/{}", repo.url, asset.id);

                yield Ok(HttpFile::new(format!("{}/{}/{}", &repo.full_name, &release.tag_name, &asset.name), asset_url)
                    .with_content_type(Some("application/octet-stream".to_string()))
                    .with_credentials(match &policy.credentials {
                      Credentials::Token(token) => Credentials::UsernamePassword {
                        username: token.clone(),
                        password: "".to_string(),
                      },
                      creds => creds.clone(),
                    })
                    .with_last_modified(Some(asset.updated_at))
                    .with_metadata_source(&repo)
                    .with_metadata_source(&release)
                    .with_metadata_source(asset));
              }
            }
          }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use rstest::rstest;

    use crate::{BackupPolicy, BackupSource};

    use super::GitHubReleasesSource;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    #[test]
    fn check_name() {
        assert_eq!(GitHubReleasesSource::default().kind(), "github/release");
    }

    #[rstest]
    #[case("users/notheotherben", true)]
    #[case("orgs/sierrasoftworks", true)]
    #[case("notheotherben", false)]
    #[case("sierrasoftworks/github-backup", false)]
    #[case("users/notheotherben/repos", false)]
    fn validation(#[case] from: &str, #[case] success: bool) {
        let source = GitHubReleasesSource::default();

        let policy = serde_yaml::from_str(&format!(
            r#"
        kind: github/release
        from: {}
        to: /tmp
        "#,
            from
        ))
        .expect("parse policy");

        if success {
            source.validate(&policy).expect("validation to succeed");
        } else {
            source.validate(&policy).expect_err("validation to fail");
        }
    }

    #[rstest]
    #[case("users/notheotherben")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn get_releases(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let source = GitHubReleasesSource::default();

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
          kind: github/release
          from: {}
          to: /tmp
          credentials: {}
        "#,
            target,
            std::env::var("GITHUB_TOKEN")
                .map(|t| format!("!Token {t}"))
                .unwrap_or_else(|_| "!None".to_string())
        ))
        .unwrap();

        println!("Using credentials: {}", policy.credentials);

        let stream = source.load(tracing::info_span!("test"), &policy, &CANCEL);
        tokio::pin!(stream);

        while let Some(release) = stream.next().await {
            println!("{}", release.expect("Failed to load release"));
        }
    }
}
