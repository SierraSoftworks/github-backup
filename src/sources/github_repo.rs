use std::sync::atomic::AtomicBool;

use tokio_stream::Stream;

use crate::{
    entities::GitRepo,
    errors::{self},
    helpers::{
        github::GitHubRepo,
        github::{GitHubArtifactKind, GitHubRepoSourceKind},
        GitHubClient,
    },
    policy::BackupPolicy,
    BackupSource,
};

#[derive(Clone)]
pub struct GitHubRepoSource {
    client: GitHubClient,
    artifact_kind: GitHubArtifactKind,
}

impl BackupSource<GitRepo> for GitHubRepoSource {
    fn kind(&self) -> &str {
        self.artifact_kind.as_str()
    }

    fn validate(&self, policy: &BackupPolicy) -> Result<(), crate::Error> {
        let target: GitHubRepoSourceKind = policy.from.as_str().parse()?;

        match target {
            GitHubRepoSourceKind::Org(_) if self.artifact_kind == GitHubArtifactKind::Star => return Err(errors::user(
              "You cannot use an organization as the source for a starred repository backup.",
              "Either use `from: user` or `from: users/<name>` when using a github/stars source kind.",
            )),
            GitHubRepoSourceKind::Repo(_) if self.artifact_kind == GitHubArtifactKind::Star => return Err(errors::user(
              "You cannot use a repository as the source for a starred repository backup.",
              "Either use `from: user` or `from: users/<name>` when using a github/stars source kind.",
            )),
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
            _ => Ok(()),
        }
    }

    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<GitRepo, errors::Error>> + 'a {
        let target: GitHubRepoSourceKind = policy.from.as_str().parse().unwrap();
        let url = format!(
            "{}/{}?{}",
            policy
                .properties
                .get("api_url")
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            target.api_endpoint(self.artifact_kind),
            policy.properties.get("query").unwrap_or(&"".to_string())
        )
        .trim_end_matches('?')
        .to_string();

        tracing_batteries::prelude::debug!("Calling {} to fetch repos", &url);

        async_stream::try_stream! {
          if matches!(target, GitHubRepoSourceKind::Repo(_)) {
            let repo = self.client.get::<GitHubRepo>(url, &policy.credentials, cancel).await?;
            yield GitRepo::new(repo.full_name.as_str(), repo.clone_url.as_str())
                .with_credentials(policy.credentials.clone())
                .with_metadata_source(&repo);
          } else {
            for await repo in self.client.get_paginated::<GitHubRepo>(url, &policy.credentials, cancel) {
              let repo = repo?;
              yield GitRepo::new(repo.full_name.as_str(), repo.clone_url.as_str())
                  .with_credentials(policy.credentials.clone())
                  .with_metadata_source(&repo);
            }
          }
        }
    }
}

impl GitHubRepoSource {
    #[allow(dead_code)]
    pub fn with_client(client: GitHubClient, kind: GitHubArtifactKind) -> Self {
        GitHubRepoSource {
            client,
            artifact_kind: kind,
        }
    }

    pub fn repo() -> Self {
        GitHubRepoSource {
            client: GitHubClient::default(),
            artifact_kind: GitHubArtifactKind::Repo,
        }
    }

    pub fn star() -> Self {
        GitHubRepoSource {
            client: GitHubClient::default(),
            artifact_kind: GitHubArtifactKind::Star,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use rstest::rstest;

    use crate::{helpers::github::GitHubArtifactKind, BackupPolicy, BackupSource};

    use super::GitHubRepoSource;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    #[test]
    fn check_name_repo() {
        assert_eq!(
            GitHubRepoSource::repo().kind(),
            GitHubArtifactKind::Repo.as_str()
        );
    }

    #[test]
    fn check_name_star() {
        assert_eq!(
            GitHubRepoSource::star().kind(),
            GitHubArtifactKind::Star.as_str()
        );
    }

    #[rstest]
    #[case("user", true)]
    #[case("users/notheotherben", true)]
    #[case("orgs/sierrasoftworks", true)]
    #[case("notheotherben", false)]
    #[case("sierrasoftworks/github-backup", false)]
    #[case("users/notheotherben/repos", false)]
    fn validation_repo(#[case] from: &str, #[case] success: bool) {
        let source = GitHubRepoSource::repo();

        let policy = serde_yaml::from_str(&format!(
            r#"
            kind: github/repo
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
    #[case("user", true)]
    #[case("users/notheotherben", true)]
    #[case("orgs/sierrasoftworks", false)]
    fn validation_stars(#[case] from: &str, #[case] success: bool) {
        let source = GitHubRepoSource::star();

        let policy = serde_yaml::from_str(&format!(
            r#"
            kind: github/star
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
    async fn get_repos(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let source = GitHubRepoSource::repo();

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
          kind: github/repo
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

        while let Some(repo) = stream.next().await {
            println!("{}", repo.expect("Failed to load repo"));
        }
    }

    #[rstest]
    #[case("users/notheotherben")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn get_stars(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let source = GitHubRepoSource::star();

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
          kind: github/star
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

        while let Some(repo) = stream.next().await {
            println!("{}", repo.expect("Failed to load repo"));
        }
    }
}
