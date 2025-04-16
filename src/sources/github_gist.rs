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

#[derive(Clone)]
pub struct GitHubGistSource {
    client: GitHubClient,
    artifact_kind: GitHubArtifactKind,
}

impl BackupSource<GitRepo> for GitHubGistSource {
    fn kind(&self) -> &str {
        self.artifact_kind.as_str()
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
            GitHubRepoSourceKind::Gist(gist) if gist.is_empty() => Err(errors::user(
                &format!(
                    "Your 'from' target '{}' is not a fully qualified GitHub gist name.",
                    policy.from.as_str()
                ),
                "Make sure you provide a fully qualified GitHub gist name in the 'from' field of your policy.",
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
    pub fn with_client(client: GitHubClient, kind: GitHubArtifactKind) -> Self {
        GitHubGistSource {
            client,
            artifact_kind: kind,
        }
    }

    pub fn gist() -> Self {
        GitHubGistSource {
            client: GitHubClient::default(),
            artifact_kind: GitHubArtifactKind::Gist,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use rstest::rstest;

    use crate::{helpers::github::GitHubArtifactKind, BackupPolicy, BackupSource};
    use crate::helpers::GitHubClient;
    use super::GitHubGistSource;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    #[test]
    fn check_name_gist() {
        assert_eq!(
            GitHubGistSource::gist().kind(),
            GitHubArtifactKind::Gist.as_str()
        );
    }

    #[rstest]
    #[case("user", true)]
    #[case("users/notheotherben", true)]
    #[case("gists/d4caf959fb7824a9855c", true)]
    #[case("starred", true)]
    fn validation_gist(#[case] from: &str, #[case] success: bool) {
        let source = GitHubGistSource::gist();

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
    #[case("user")]
    #[case("users/notheotherben")]
    #[case("starred")]
    #[case("gists/aa5a315d61ae9438b18d")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn get_gist_repos(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let source = GitHubGistSource::with_client(
            GitHubClient::default()
                .mock("/gists", |b| b.with_body_from_file("github.gists.0.json"))
                .mock("/users/notheotherben/gists", |b| b.with_body_from_file("github.gists.0.json"))
                .mock("/gists/starred", |b| b.with_body_from_file("github.gists.0.json"))
                .mock("/gists/aa5a315d61ae9438b18d", |b| b.with_body_from_file("github.gists.1.json")),

            GitHubArtifactKind::Gist,
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

        while let Some(repo) = stream.next().await {
            match repo {
                Ok(r) => {
                    println!("{}", r)
                }
                Err(e) => {
                    panic!("{}", e)
                }
            }
        }
    }
}
