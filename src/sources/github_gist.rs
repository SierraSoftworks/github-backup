use std::sync::atomic::AtomicBool;

use tokio_stream::Stream;

use crate::helpers::github::GitHubGist;
use crate::{
    entities::GitRepo,
    errors::{self},
    helpers::{
        github::{GitHubArtifactKind, GitHubRepoSourceKind},
        GitHubClient,
    },
    policy::BackupPolicy,
    BackupSource,
};

#[derive(Clone, Default)]
pub struct GitHubGistSource {
    client: GitHubClient,
}

impl BackupSource<GitRepo> for GitHubGistSource {
    fn kind(&self) -> &str {
        GitHubArtifactKind::Gist.as_str()
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
            target.api_endpoint(GitHubArtifactKind::Gist),
            policy.properties.get("query").unwrap_or(&"".to_string())
        )
        .trim_end_matches('?')
        .to_string();

        tracing_batteries::prelude::debug!("Calling {} to fetch gists", &url);

        let refspecs = policy
            .properties
            .get("refspecs")
            .map(|r| r.split(',').map(|r| r.to_string()).collect::<Vec<String>>());

        async_stream::try_stream! {
          if matches!(target, GitHubRepoSourceKind::Gist(_)) {
            let gist: GitHubGist = self.client.get(&url, &policy.credentials, cancel).await?;
            yield GitRepo::new(
              gist.id.as_str(),
              gist.git_pull_url.as_str(),
              refspecs.clone())
                .with_credentials(policy.credentials.clone())
                .with_metadata_source(&gist);
          } else {
            for await gist in self.client.get_paginated(&url, &policy.credentials, cancel) {
              let gist: GitHubGist = gist?;
              yield GitRepo::new(
                gist.id.as_str(),
                gist.git_pull_url.as_str(),
                refspecs.clone())
                  .with_credentials(policy.credentials.clone())
                  .with_metadata_source(&gist);
            }
          }
        }
    }
}

impl GitHubGistSource {
    #[allow(dead_code)]
    pub fn with_client(client: GitHubClient) -> Self {
        GitHubGistSource { client }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use rstest::rstest;

    use super::GitHubGistSource;
    use crate::helpers::GitHubClient;
    use crate::{helpers::github::GitHubArtifactKind, BackupPolicy, BackupSource};

    static CANCEL: AtomicBool = AtomicBool::new(false);

    #[test]
    fn check_name_gist() {
        assert_eq!(
            GitHubGistSource::default().kind(),
            GitHubArtifactKind::Gist.as_str()
        );
    }

    #[rstest]
    #[case("user", true)]
    #[case("user/", false)]
    #[case("users/notheotherben", true)]
    #[case("gists/d4caf959fb7824a9855c", true)]
    #[case("gists/", false)]
    #[case("starred", true)]
    fn validation_gist(#[case] from: &str, #[case] success: bool) {
        let source = GitHubGistSource::default();

        let policy = serde_yaml::from_str(&format!(
            r#"
            kind: github/gist
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
    #[case("user", "/gists", "github.gists.0.json", 2)]
    #[case("users/octocat", "/users/octocat/gists", "github.gists.0.json", 2)]
    #[case("starred", "/gists/starred", "github.gists.0.json", 2)]
    #[case(
        "gists/aa5a315d61ae9438b18d",
        "/gists/aa5a315d61ae9438b18d",
        "github.gists.1.json",
        1
    )]
    #[tokio::test]
    async fn get_gist_repos(
        #[case] target: &str,
        #[case] api_endpoint: &str,
        #[case] filename: &str,
        #[case] expected_entries: usize,
    ) {
        use tokio_stream::StreamExt;

        let source = GitHubGistSource::with_client(
            GitHubClient::default().mock(api_endpoint, |b| b.with_body_from_file(filename)),
        );

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
          kind: github/gist
          from: {}
          to: /tmp
        "#,
            target
        ))
        .unwrap();

        let stream = source.load(&policy, &CANCEL);
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(gist) = stream.next().await {
            match gist {
                Ok(_) => {}
                Err(e) => {
                    panic!("{}", e)
                }
            }
            count += 1;
        }

        assert_eq!(
            count, expected_entries,
            "Expected {} entries, got {}",
            expected_entries, count
        );
    }
}
