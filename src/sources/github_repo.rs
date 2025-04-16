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

#[derive(Clone, Default)]
pub struct GitHubRepoSource {
    client: GitHubClient,
}

impl BackupSource<GitRepo> for GitHubRepoSource {
    fn kind(&self) -> &str {
        GitHubArtifactKind::Repo.as_str()
    }

    fn validate(&self, policy: &BackupPolicy) -> Result<(), crate::Error> {
        let _: GitHubRepoSourceKind = policy.from.as_str().parse()?;
        Ok(())
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
            target.api_endpoint(GitHubArtifactKind::Repo),
            policy.properties.get("query").unwrap_or(&"".to_string())
        )
        .trim_end_matches('?')
        .to_string();

        tracing_batteries::prelude::debug!("Calling {} to fetch repos", &url);

        let refspecs = policy
            .properties
            .get("refspecs")
            .map(|r| r.split(',').map(|r| r.to_string()).collect::<Vec<String>>());

        async_stream::try_stream! {
          if matches!(target, GitHubRepoSourceKind::Repo(_)) {
            let repo: GitHubRepo = self.client.get(&url, &policy.credentials, cancel).await?;
            yield GitRepo::new(
              repo.full_name.as_str(),
              repo.clone_url.as_str(),
              refspecs.clone())
                .with_credentials(policy.credentials.clone())
                .with_metadata_source(&repo);
          } else {
            for await repo in self.client.get_paginated(&url, &policy.credentials, cancel) {
              let repo: GitHubRepo = repo?;
              yield GitRepo::new(
                repo.full_name.as_str(),
                repo.clone_url.as_str(),
                refspecs.clone())
                  .with_credentials(policy.credentials.clone())
                  .with_metadata_source(&repo);
            }
          }
        }
    }
}

impl GitHubRepoSource {
    #[allow(dead_code)]
    pub fn with_client(client: GitHubClient) -> Self {
        GitHubRepoSource { client }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use rstest::rstest;

    use super::GitHubRepoSource;
    use crate::helpers::GitHubClient;
    use crate::{helpers::github::GitHubArtifactKind, BackupPolicy, BackupSource};

    static CANCEL: AtomicBool = AtomicBool::new(false);

    #[test]
    fn check_name_repo() {
        assert_eq!(
            GitHubRepoSource::default().kind(),
            GitHubArtifactKind::Repo.as_str()
        );
    }

    #[rstest]
    #[case("user", true)]
    #[case("users/ ", false)]
    #[case("users/notheotherben", true)]
    #[case("orgs/sierrasoftworks", true)]
    #[case("notheotherben", false)]
    #[case("sierrasoftworks/github-backup", false)]
    #[case("users/notheotherben/repos", false)]
    #[case("starred", true)]
    fn validation_repo(#[case] from: &str, #[case] success: bool) {
        let source = GitHubRepoSource::default();

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
    #[case("users/notheotherben")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn get_repos(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let source = GitHubRepoSource::default();

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
    #[case("users/octocat", "/users/octocat/repos", "github.repos.0.json", 31)]
    #[case("starred", "/user/starred", "github.repos.1.json", 2)]
    #[tokio::test]
    async fn get_repos_mocked(
        #[case] target: &str,
        #[case] api_endpoint: &str,
        #[case] filename: &str,
        #[case] expected_entries: usize,
    ) {
        use tokio_stream::StreamExt;

        let source = GitHubRepoSource::with_client(
            GitHubClient::default().mock(api_endpoint, |b| b.with_body_from_file(filename)),
        );

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
          kind: github/repo
          from: {}
          to: /tmp
        "#,
            target
        ))
        .unwrap();

        let stream = source.load(&policy, &CANCEL);
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(repo) = stream.next().await {
            println!("{}", repo.expect("Failed to load repo"));
            count += 1;
        }

        assert_eq!(
            count, expected_entries,
            "Expected {} entries, got {}",
            expected_entries, count
        );
    }
}
