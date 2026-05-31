#[derive(Clone, Debug, PartialEq)]
pub enum GitHubRepoSourceKind {
    CurrentUser,
    User(String),
    Org(String),
    Starred,
    Repo(String),
    Gist(String),
}

impl GitHubRepoSourceKind {
    pub fn api_endpoint(&self, artifact_kind: GitHubArtifactKind) -> String {
        match self {
            GitHubRepoSourceKind::CurrentUser => match artifact_kind {
                GitHubArtifactKind::Gist => artifact_kind.api_endpoint().to_string(),
                _ => format!("user/{}", artifact_kind.api_endpoint()),
            },
            GitHubRepoSourceKind::User(u) => {
                format!("users/{}/{}", u, artifact_kind.api_endpoint())
            }
            GitHubRepoSourceKind::Org(o) => format!("orgs/{}/{}", o, artifact_kind.api_endpoint()),
            GitHubRepoSourceKind::Repo(r) => format!("repos/{}", r),
            GitHubRepoSourceKind::Gist(g) => format!("gists/{}", g),
            GitHubRepoSourceKind::Starred => match artifact_kind {
                GitHubArtifactKind::Gist => "gists/starred".to_string(),
                _ => "user/starred".to_string(),
            },
        }
    }
}

impl std::str::FromStr for GitHubRepoSourceKind {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split('/').collect::<Vec<&str>>().as_slice() {
            ["user"] => Ok(GitHubRepoSourceKind::CurrentUser),
            ["starred"] => Ok(GitHubRepoSourceKind::Starred),
            ["users", user] if !user.is_empty() => Ok(GitHubRepoSourceKind::User(user.to_string())),
            ["orgs", org] if !org.is_empty() => Ok(GitHubRepoSourceKind::Org(org.to_string())),
            ["repos", owner, repo] if !repo.is_empty() => {
                Ok(GitHubRepoSourceKind::Repo(format!("{owner}/{repo}")))
            }
            ["gists", gist] if !gist.is_empty() => Ok(GitHubRepoSourceKind::Gist(gist.to_string())),
            _ => Err(human_errors::user(
                format!(
                    "The 'from' declaration '{}' was not valid for a GitHub repository source.",
                    s
                ),
                &[
                    "Make sure you provide either 'user', 'users/<name>', 'orgs/<name>', or 'repos/<owner>/<name>'",
                ],
            )),
        }
    }
}

#[allow(dead_code)]
#[derive(PartialEq, Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub enum GitHubArtifactKind {
    #[serde(rename = "github/repo")]
    Repo,
    #[serde(rename = "github/release")]
    Release,
    #[serde(rename = "github/gist")]
    Gist,
}

impl GitHubArtifactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            GitHubArtifactKind::Repo => "github/repo",
            GitHubArtifactKind::Release => "github/release",
            GitHubArtifactKind::Gist => "github/gist",
        }
    }

    pub fn api_endpoint(&self) -> &'static str {
        match self {
            GitHubArtifactKind::Repo => "repos",
            GitHubArtifactKind::Release => "repos",
            GitHubArtifactKind::Gist => "gists",
        }
    }
}
