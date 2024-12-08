use std::sync::atomic::AtomicBool;

use tokio_stream::Stream;

use crate::{
    entities::GitRepo,
    errors::{self},
    helpers::{github::GitHubArtifactKind, github::GitHubRepo, GitHubClient},
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

            t if t == "user" => Ok(()),
            t if t.starts_with("users/") => Ok(()),

            t if t.starts_with("orgs/") && self.artifact_kind == GitHubArtifactKind::Star => Err(errors::user(
                &format!("The target field '{target}' specifies an org which is not support for kind 'github/star'."),
                "Please specify either 'users/<username>' as your target when using 'github/star' as kind.",
            )),
            t if t.starts_with("orgs/") => Ok(()),

            _ => Err(errors::user(
                &format!("The target field '{target}' does not include a valid user or org specifier."),
                "Please specify either 'user', 'users/<username>' or 'orgs/<orgname>' as your target.",
            )),
        }
    }

    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<GitRepo, errors::Error>> + 'a {
        let url = format!(
            "{}/{}/{}?{}",
            policy
                .properties
                .get("api_url")
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            &policy.from.trim_matches('/'),
            self.artifact_kind.api_endpoint(),
            policy.properties.get("query").unwrap_or(&"".to_string())
        );

        async_stream::try_stream! {
          for await repo in self.client.get_paginated::<GitHubRepo>(url, &policy.credentials, cancel) {
            let repo = repo?;
            yield GitRepo::new(repo.full_name.as_str(), repo.clone_url.as_str())
                .with_credentials(policy.credentials.clone())
                .with_metadata_source(&repo);
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
