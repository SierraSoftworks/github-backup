use std::sync::atomic::AtomicBool;

use tokio_stream::Stream;

use crate::{
    BackupSource,
    entities::{Credentials, HttpFile, Release},
    helpers::{
        GitHubClient,
        github::{GitHubArtifactKind, GitHubRelease, GitHubRepo, GitHubRepoSourceKind},
    },
    policy::BackupPolicy,
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
    ) -> impl Stream<Item = Result<Release, crate::Error>> + 'a {
        async_stream::stream! {
          if !repo.has_downloads {
            return;
          }

          let releases_url = format!("{}/releases", repo.url);

          for await release in self.client.get_paginated(&releases_url, &policy.credentials, cancel) {
            let release: GitHubRelease = match release {
              Ok(release) => release,
              Err(e) => {
                yield Err(e);
                continue;
              }
            };

            let mut entity = Release::new(
                format!("{}/{}", repo.full_name, release.tag_name),
                repo.full_name.as_str(),
                release.tag_name.as_str(),
              )
              .with_body(release.body.clone())
              .with_draft(release.draft)
              .with_prerelease(release.prerelease)
              .with_metadata_source(repo)
              .with_metadata_source(&release);

            if let Some(tarball_url) = &release.tarball_url {
              entity = entity.with_asset(
                HttpFile::new(format!("{}/{}/source.tar.gz", repo.full_name, release.tag_name), tarball_url)
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

              entity = entity.with_asset(
                HttpFile::new(format!("{}/{}/{}", repo.full_name, release.tag_name, asset.name), asset_url)
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

            // Apply the policy filter at the granularity of individual release
            // assets, preserving the ability to control backups using the
            // `asset.*`, `release.*` and `repo.*` filter fields.
            let mut assets = Vec::with_capacity(entity.assets.len());
            for asset in std::mem::take(&mut entity.assets) {
              match policy.filter.matches(&asset) {
                Ok(true) => assets.push(asset),
                Ok(false) => {},
                Err(e) => {
                  yield Err(e);
                }
              }
            }
            entity.assets = assets;

            // The release notes are governed by the release-level fields
            // (`repo.*` / `release.*`); drop them when the release is filtered
            // out at that level.
            match policy.filter.matches(&entity) {
              Ok(true) => {},
              Ok(false) => { entity.body = None; },
              Err(e) => {
                yield Err(e);
                continue;
              }
            }

            // Skip releases which have nothing left to back up after filtering.
            if entity.assets.is_empty() && entity.body.as_deref().is_none_or(str::is_empty) {
              continue;
            }

            yield Ok(entity);
          }
        }
    }
}

impl BackupSource<Release> for GitHubReleasesSource {
    fn kind(&self) -> &str {
        GitHubArtifactKind::Release.as_str()
    }

    fn filters_internally(&self) -> bool {
        true
    }

    fn validate(&self, policy: &BackupPolicy) -> Result<(), human_errors::Error> {
        let target: GitHubRepoSourceKind = policy.from.as_str().parse()?;

        match target {
            GitHubRepoSourceKind::Starred => Err(human_errors::user(
                format!(
                    "You cannot use 'from: {}' for backups of 'kind: {}' as it is not currently supported.",
                    policy.from.as_str(),
                    policy.kind.as_str()
                ),
                &[
                    "Try using 'from: user' or one of the other supported sources (users/<user>, orgs/<org>, repos/<repo>, etc).",
                ],
            )),
            _ => Ok(()),
        }
    }

    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<Release, human_errors::Error>> + 'a {
        async_stream::stream! {
          let target: GitHubRepoSourceKind = match policy.from.as_str().parse() {
            Ok(target) => target,
            Err(e) => {
              yield Err(e);
              return;
            }
          };
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

          if matches!(target, GitHubRepoSourceKind::Repo(_)) {
            let repo: GitHubRepo = self.client.get(&url, &policy.credentials, cancel).await?;

            for await file in self.load_releases(policy, &repo, cancel) {
              yield file;
            }
          } else {
            for await repo in self.client.get_paginated(&url, &policy.credentials, cancel) {
              let repo: GitHubRepo = match repo {
                Ok(repo) => repo,
                Err(e) => {
                  yield Err(e);
                  continue;
                }
              };

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
    #[case("github.releases.0.json", 31)]
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
            let release = release.expect("Failed to load release");
            println!("{}", release);

            // Each release in the fixture bundles a tarball plus two uploaded
            // assets and includes a set of release notes.
            assert_eq!(release.assets.len(), 3);
            assert!(release.body.is_some());

            count += 1;
        }

        assert_eq!(
            count, expected_entries,
            "Expected {} entries, got {}",
            expected_entries, count
        );
    }

    #[rstest]
    // Only the matching asset is retained, and the release notes are dropped
    // because the release does not match at the `release.*` / `repo.*` level.
    #[case("asset.name == \"client.exe\"", 31, 1, false)]
    // A `release.*` filter keeps every asset and the release notes.
    #[case("release.prerelease == false", 31, 3, true)]
    // A filter which matches no assets skips the release entirely.
    #[case("asset.name == \"does-not-exist\"", 0, 0, false)]
    #[tokio::test]
    async fn get_releases_filtered(
        #[case] filter: &str,
        #[case] expected_releases: usize,
        #[case] expected_assets: usize,
        #[case] expect_notes: bool,
    ) {
        use tokio_stream::StreamExt;

        let source = GitHubReleasesSource::with_client(
            GitHubClient::default()
                .mock("/users/octocat/repos", |b| {
                    b.with_body_from_file("github.repos.0.json")
                })
                .mock("/repos/octocat/repo/releases", |b| {
                    b.with_body_from_file("github.releases.0.json")
                }),
        );

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
          kind: github/release
          from: users/octocat
          to: /tmp
          filter: '{filter}'
        "#
        ))
        .unwrap();

        let stream = source.load(&policy, &CANCEL);
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(release) = stream.next().await {
            let release = release.expect("Failed to load release");
            assert_eq!(
                release.assets.len(),
                expected_assets,
                "unexpected asset count for filter '{filter}'"
            );
            assert_eq!(
                release.body.is_some(),
                expect_notes,
                "unexpected release notes presence for filter '{filter}'"
            );

            count += 1;
        }

        assert_eq!(
            count, expected_releases,
            "Expected {} releases for filter '{}', got {}",
            expected_releases, filter, count
        );
    }
}
