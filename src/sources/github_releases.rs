use std::sync::atomic::AtomicBool;

use tokio_stream::Stream;

use crate::{
    entities::{Credentials, HttpFile},
    errors::{self},
    helpers::{
        github::{GitHubArtifactKind, GitHubRelease, GitHubRepo, GitHubRepoSourceKind},
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

impl GitHubReleasesSource {
    fn load_releases<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        repo: &'a GitHubRepo,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<HttpFile, crate::Error>> + 'a {
        async_stream::stream! {
          if !repo.has_downloads {
            return;
          }

          let releases_url = format!("{}/releases", repo.url);

          for await release in self.client.get_paginated(&releases_url, &policy.credentials, cancel) {
            if let Err(e) = release {
              yield Err(e);
              continue;
            }

            let release: GitHubRelease = release.unwrap();

            if let Some(tarball_url) = &release.tarball_url {
              yield Ok(HttpFile::new(format!("{}/{}/source.tar.gz", &repo.full_name, &release.tag_name), tarball_url)
                  .with_metadata_source(repo)
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
                  .with_metadata_source(repo)
                  .with_metadata_source(&release)
                  .with_metadata_source(asset));
            }
          }
        }
    }
}

impl BackupSource<HttpFile> for GitHubReleasesSource {
    fn kind(&self) -> &str {
        GitHubArtifactKind::Release.as_str()
    }

    fn validate(&self, policy: &BackupPolicy) -> Result<(), crate::Error> {
        let target: GitHubRepoSourceKind = policy.from.as_str().parse()?;

        match target {
          GitHubRepoSourceKind::User(u) if u.is_empty() => Err(errors::user(
              &format!(
                  "Your 'from' target '{}' is not a valid GitHub username.",
                  policy.from.as_str()
              ),
              "Make sure you provide a valid GitHub username in the 'from' field of your policy.",
          )),
          GitHubRepoSourceKind::Org(org) if org.is_empty() => Err(errors::user(
              &format!(
                  "Your 'from' target '{}' is not a valid GitHub organization name.",
                  policy.from.as_str()
              ),
              "Make sure you provide a valid GitHub organization name in the 'from' field of your policy.",
          )),
          GitHubRepoSourceKind::Repo(repo) if repo.is_empty() => Err(errors::user(
              &format!(
                  "Your 'from' target '{}' is not a fully qualified GitHub repository name.",
                  policy.from.as_str()
              ),
              "Make sure you provide a fully qualified GitHub repository name in the 'from' field of your policy.",
          )),
            GitHubRepoSourceKind::Starred => Err(errors::user(
                &format!(
                    "Your 'from' target '{}' is not valid for 'kind' '{}'.",
                    policy.from.as_str(),
                    policy.kind.as_str()
                ),
                "You cannot use starred to backup releases.",
            )),
          _ => Ok(()),
      }
    }

    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<HttpFile, crate::Error>> + 'a {
        let target: GitHubRepoSourceKind = policy.from.as_str().parse().unwrap();
        let url = format!(
            "{}/{}?{}",
            policy
                .properties
                .get("api_url")
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            target.api_endpoint(GitHubArtifactKind::Release),
            policy.properties.get("query").unwrap_or(&"".to_string())
        )
        .trim_end_matches('?')
        .to_string();

        async_stream::stream! {
          if matches!(target, GitHubRepoSourceKind::Repo(_)) {
            let repo: GitHubRepo = self.client.get(&url, &policy.credentials, cancel).await?;

            for await file in self.load_releases(policy, &repo, cancel) {
              yield file;
            }
          } else {
            for await repo in self.client.get_paginated(&url, &policy.credentials, cancel) {
              if let Err(e) = repo {
                yield Err(e);
                continue;
              }

              let repo: GitHubRepo = repo.unwrap();

              for await file in self.load_releases(policy, &repo, cancel) {
                yield file;
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

    use super::GitHubReleasesSource;
    use crate::helpers::GitHubClient;
    use crate::{BackupPolicy, BackupSource};

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
    #[case("starred", false)]
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

        let stream = source.load(&policy, &CANCEL);
        tokio::pin!(stream);

        while let Some(release) = stream.next().await {
            println!("{}", release.expect("Failed to load release"));
        }
    }

    #[rstest]
    #[case("github.releases.0.json", 93)]
    #[tokio::test]
    async fn get_releases_mocked(#[case] filename: &str, #[case] expected_entries: usize) {
        use tokio_stream::StreamExt;

        let source = GitHubReleasesSource::with_client(
            GitHubClient::default()
                .mock("/users/octocat/repos", |b| {
                    b.with_body_from_file("github.repos.0.json")
                })
                .mock("/repos/octocat/repo/releases", |b| {
                    b.with_body_from_file(filename)
                }),
        );

        let policy: BackupPolicy = serde_yaml::from_str(
            r#"
          kind: github/release
          from: users/octocat
          to: /tmp
        "#,
        )
        .unwrap();

        let stream = source.load(&policy, &CANCEL);
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(release) = stream.next().await {
            println!("{}", release.expect("Failed to load release"));
            count += 1;
        }

        assert_eq!(
            count, expected_entries,
            "Expected {} entries, got {}",
            expected_entries, count
        );
    }
}
