mod client;
mod entities;
mod types;

pub use client::GitHubClient;
pub use entities::{GitHubGist, GitHubRelease, GitHubRepo};
pub use types::{GitHubArtifactKind, GitHubRepoSourceKind};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::entities::{Credentials, Metadata, MetadataSource};
    use rstest::rstest;
    use serde::de::DeserializeOwned;
    use tokio_stream::StreamExt;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    fn load_test_file<T: DeserializeOwned>(name: &str) -> Result<T, Box<dyn std::error::Error>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join(name);
        let json = std::fs::read_to_string(path)?;
        let value = serde_json::from_str(&json)?;
        Ok(value)
    }

    #[tokio::test]
    async fn test_mock_mode() {
        let client = GitHubClient::default().mock("/users/notheotherben/repos", |b| {
            b.with_body_from_file("github.repos.0.json")
        });

        let stream = client.get_paginated(
            "https://api.github.com/users/notheotherben/repos",
            &Credentials::None,
            &CANCEL,
        );
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(repo) = stream.next().await {
            let repo: GitHubRepo = repo.expect("Failed to fetch repo");
            assert!(!repo.name.is_empty());
            count += 1;
        }

        assert!(count > 0, "at least one repo should be returned");
    }

    #[rstest]
    #[case("github.repos.0.json", 31)]
    fn test_deserialize_repos(#[case] file: &str, #[case] repo_count: usize) {
        let repos: Vec<GitHubRepo> = load_test_file(file).expect("Failed to load test file");
        assert_eq!(repos.len(), repo_count);

        for repo in repos {
            let mut metadata = Metadata::default();
            repo.inject_metadata(&mut metadata);

            assert_eq!(metadata.get("repo.name"), repo.name.into());
            assert_eq!(metadata.get("repo.fullname"), repo.full_name.into());
            assert_eq!(metadata.get("repo.private"), repo.private.into());
            assert_eq!(metadata.get("repo.fork"), repo.fork.into());
            assert_eq!(metadata.get("repo.archived"), repo.archived.into());
            assert_eq!(metadata.get("repo.disabled"), repo.disabled.into());
            assert_eq!(metadata.get("repo.empty"), (repo.size == 0).into());
        }
    }

    #[rstest]
    #[case("github.gists.0.json", 2)]
    fn test_deserialize_gist(#[case] file: &str, #[case] repo_count: usize) {
        let gists: Vec<GitHubGist> = load_test_file(file).expect("Failed to load test file");
        assert_eq!(gists.len(), repo_count);

        for gist in gists {
            let mut metadata = Metadata::default();
            gist.inject_metadata(&mut metadata);

            assert_eq!(metadata.get("gist.public"), gist.public.into());
            assert_eq!(
                metadata.get("gist.comments_enabled"),
                gist.comments_enabled.into()
            );
            assert_eq!(metadata.get("gist.comments"), gist.comments.into());
        }
    }

    #[rstest]
    #[case("github.releases.0.json", 1)]
    #[case("github.releases.1.json", 8)]
    fn test_deserialize_releases(#[case] file: &str, #[case] release_count: usize) {
        let releases: Vec<GitHubRelease> = load_test_file(file).expect("Failed to load test file");
        assert_eq!(releases.len(), release_count);

        for release in releases {
            let mut metadata = Metadata::default();
            release.inject_metadata(&mut metadata);

            assert_eq!(metadata.get("release.tag"), release.tag_name.into());
            assert_eq!(metadata.get("release.name"), release.name.into());
            assert_eq!(metadata.get("release.draft"), release.draft.into());
            assert_eq!(
                metadata.get("release.prerelease"),
                release.prerelease.into()
            );
        }
    }

    #[rstest]
    #[case("users/notheotherben")]
    #[case("orgs/sierrasoftworks")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn fetch_repos(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let client = GitHubClient::default();
        let creds = get_test_credentials();

        let stream = client.get_paginated(
            format!("https://api.github.com/{target}/repos"),
            &creds,
            &CANCEL,
        );
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(repo) = stream.next().await {
            let repo: GitHubRepo = repo.expect("Failed to fetch repo");
            assert!(!repo.name.is_empty());
            count += 1;
        }

        assert!(count > 0, "at least one repo should be returned");
    }

    #[rstest]
    #[case("users/cedi")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn fetch_gist(#[case] target: &str) {
        use tokio_stream::StreamExt;

        let client = GitHubClient::default();
        let creds = get_test_credentials();

        let stream = client.get_paginated(
            format!("https://api.github.com/{target}/gists"),
            &creds,
            &CANCEL,
        );
        tokio::pin!(stream);

        let mut count = 0;
        while let Some(gist) = stream.next().await {
            let gist: GitHubGist = gist.expect("Failed to fetch gist");
            assert!(!gist.id.is_empty());
            count += 1;
        }

        assert!(count > 0, "at least one gist should be returned");
    }

    #[rstest]
    #[case("sierrasoftworks/github-backup")]
    #[tokio::test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    async fn get_repo(#[case] target: &str) {
        let client = GitHubClient::default();
        let creds = get_test_credentials();

        let repo = client
            .get(
                format!("https://api.github.com/repos/{target}"),
                &creds,
                &CANCEL,
            )
            .await;
        let repo: GitHubRepo = repo.expect("Failed to fetch repo");

        assert_eq!(repo.full_name.to_lowercase(), target.to_lowercase());
    }

    fn get_test_credentials() -> Credentials {
        std::env::var("GITHUB_TOKEN")
            .map(|t| Credentials::UsernamePassword {
                username: t,
                password: String::new(),
            })
            .unwrap_or(Credentials::None)
    }

    #[rstest]
    #[case("github/repo", GitHubArtifactKind::Repo, "repos")]
    #[case("github/release", GitHubArtifactKind::Release, "repos")]
    #[case("github/gist", GitHubArtifactKind::Gist, "gists")]
    fn test_deserialize_gh_repo_kind(
        #[case] kind_str: &str,
        #[case] expected_kind: GitHubArtifactKind,
        #[case] url: &str,
    ) {
        let kind: GitHubArtifactKind = serde_yaml::from_str(&format!("\"{}\"", kind_str)).unwrap();

        assert_eq!(kind, expected_kind);
        assert_eq!(kind.as_str(), kind_str);
        assert_eq!(kind.api_endpoint(), url);
    }

    #[rstest]
    #[case(
        GitHubRepoSourceKind::CurrentUser,
        GitHubArtifactKind::Repo,
        "user/repos"
    )]
    #[case(GitHubRepoSourceKind::CurrentUser, GitHubArtifactKind::Gist, "gists")]
    #[case(GitHubRepoSourceKind::User("octocat".to_string()), GitHubArtifactKind::Repo, "users/octocat/repos")]
    #[case(GitHubRepoSourceKind::User("octocat".to_string()), GitHubArtifactKind::Gist, "users/octocat/gists")]
    #[case(GitHubRepoSourceKind::Org("octocat".to_string()), GitHubArtifactKind::Repo, "orgs/octocat/repos")]
    #[case(
        GitHubRepoSourceKind::Starred,
        GitHubArtifactKind::Repo,
        "user/starred"
    )]
    #[case(
        GitHubRepoSourceKind::Starred,
        GitHubArtifactKind::Gist,
        "gists/starred"
    )]
    #[case(GitHubRepoSourceKind::Repo("octocat".to_string()), GitHubArtifactKind::Repo, "repos/octocat")]
    fn test_source_kind_api_url(
        #[case] source_kind: GitHubRepoSourceKind,
        #[case] artifact_kind: GitHubArtifactKind,
        #[case] expected_api_url: &str,
    ) {
        assert_eq!(source_kind.api_endpoint(artifact_kind), expected_api_url);
    }

    #[rstest]
    #[case("user", GitHubRepoSourceKind::CurrentUser)]
    #[case("users/notheotherben", GitHubRepoSourceKind::User("notheotherben".into()))]
    #[case("orgs/sierrasoftworks", GitHubRepoSourceKind::Org("sierrasoftworks".into()))]
    #[case("repos/sierrasoftworks/github-backup", GitHubRepoSourceKind::Repo("sierrasoftworks/github-backup".into()))]
    #[case("gists/abcd", GitHubRepoSourceKind::Gist("abcd".into()))]
    #[case("starred", GitHubRepoSourceKind::Starred)]
    fn test_deserialize_gh_repo_source_kind(
        #[case] kind_str: &str,
        #[case] expected_kind: GitHubRepoSourceKind,
    ) {
        let kind: GitHubRepoSourceKind = kind_str.parse().unwrap();
        assert_eq!(kind, expected_kind);
    }
}
