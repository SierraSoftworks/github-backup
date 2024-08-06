use std::sync::atomic::AtomicBool;

use tokio_stream::{Stream, StreamExt};

use crate::{
    entities::GitRepo,
    errors::{self},
    helpers::{github::GitHubRepo, GitHubClient},
    policy::BackupPolicy,
    BackupSource,
};

#[derive(Clone)]
pub struct GitHubRepoSource {
    client: GitHubClient,
}

impl BackupSource<GitRepo> for GitHubRepoSource {
    fn kind(&self) -> &str {
        "github/repo"
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
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<GitRepo, errors::Error>> + 'a {
        let url = format!(
            "{}/{}/repos",
            policy
                .properties
                .get("api_url")
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            &policy.from.trim_matches('/')
        );

        self.client
            .get_paginated::<crate::helpers::github::GitHubRepo>(url, &policy.credentials, cancel)
            .map(|result| {
                result.map(|repo: GitHubRepo| {
                    GitRepo::new(repo.full_name.as_str(), repo.clone_url.as_str())
                        .with_credentials(policy.credentials.clone())
                        .with_metadata_source(&repo)
                })
            })
    }
}

impl GitHubRepoSource {
    pub fn new() -> Self {
        GitHubRepoSource {
            client: Default::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_client(client: GitHubClient) -> Self {
        GitHubRepoSource { client }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use rstest::rstest;

    use crate::{BackupPolicy, BackupSource};

    use super::GitHubRepoSource;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    #[rstest]
    #[case("users/notheotherben")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn get_repos(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let source = GitHubRepoSource::new();

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
}
