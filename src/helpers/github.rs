use std::{
    fmt::Display,
    sync::{atomic::AtomicBool, Arc},
};

use reqwest::{header::LINK, Method, StatusCode, Url};
use tokio_stream::Stream;

use crate::{
    entities::{Credentials, MetadataSource},
    errors::{self, ResponseError},
};

#[derive(Clone)]
pub struct GitHubClient {
    client: Arc<reqwest::Client>,
}

impl GitHubClient {
    #[allow(dead_code)]
    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: String,
        creds: &Credentials,
        cancel: &AtomicBool,
    ) -> Result<T, errors::Error> {
        let resp = self.call(Method::GET, &url, creds, |r| r, cancel).await?;

        resp.json().await.map_err(|e| {
            errors::system_with_internal(
                &format!(
                    "Unable to parse GitHub's response for '{}' due to invalid JSON.",
                    &url
                ),
                "Please report this issue to us on GitHub.",
                e,
            )
        })
    }

    pub fn get_paginated<'a, T: serde::de::DeserializeOwned + 'a>(
        &'a self,
        page_url: String,
        creds: &'a Credentials,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<T, errors::Error>> + 'a {
        async_stream::try_stream! {
          let mut page_url = Some(page_url);

          while let Some(url) = page_url {
              if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  Err(errors::user(
                      "The backup operation was cancelled by the user. Only partial data may have been backed up.",
                      "Allow the backup to complete fully before cancelling again."))?;
              }

              let resp = self.call(Method::GET, &url, creds, |r| r, cancel).await?;

              if let Some(link_header) = resp.headers().get(LINK) {
                  let link_header = link_header.to_str().map_err(|e| errors::system_with_internal(
                      "Unable to parse GitHub's Link header due to invalid characters, which will result in pagination failing to work correctly.",
                      "Please report this issue to us on GitHub.",
                      e))?;

                  let links = parse_link_header::parse_with_rel(link_header).map_err(|e| errors::system_with_internal(
                      "Unable to parse GitHub's Link header, which will result in pagination failing to work correctly.",
                      "Please report this issue to us on GitHub.",
                      e))?;

                  if let Some(next_link) = links.get("next") {
                      page_url = Some(next_link.raw_uri.clone());
                  } else {
                      page_url = None;
                  }
              } else {
                  page_url = None;
              }

              match resp.json::<Vec<T>>().await {
                Ok(results) => {
                  for result in results.into_iter() {
                      yield result;
                  }
                },
                Err(err) => {
                  Err(errors::system_with_internal(
                      &format!("Unable to parse GitHub response into the expected structure when requesting '{}'.", &url),
                      "Please report this issue to us on GitHub.",
                      err))?;
                }
              }
          }
        }
    }

    async fn call<B>(
        &self,
        method: Method,
        url: &str,
        creds: &Credentials,
        builder: B,
        _cancel: &AtomicBool,
    ) -> Result<reqwest::Response, errors::Error>
    where
        B: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        let parsed_url: Url = url.parse().map_err(|e| {
            errors::user_with_internal(
                &format!("Unable to parse GitHub URL '{}' as a valid URL.", &url),
                "Make sure that you have configured your GitHub API correctly.",
                e,
            )
        })?;

        let mut req = self
            .client
            .request(method, parsed_url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "SierraSoftworks/github-backup");

        req = match creds {
            Credentials::None => req,
            Credentials::Token(token) => req.basic_auth(token, Some("".to_string())),
            Credentials::UsernamePassword { username, password } => {
                req.basic_auth(username, Some(password))
            }
        };

        let req = builder(req);

        let resp = req.send().await?;

        if resp.status().is_success() {
            Ok(resp)
        } else if resp.status() == StatusCode::UNAUTHORIZED {
            Err(errors::user(
                "The access token you have provided was rejected by the GitHub API.",
                "Make sure that your GitHub token is valid and has not expired.",
            ))
        } else {
            let err = ResponseError::with_body(resp).await;
            Err(errors::user_with_internal(
                &format!(
                    "The GitHub API returned an error response with status code {}.",
                    err.status_code
                ),
                "Please check the error message below and try again.",
                err,
            ))
        }
    }
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self {
            client: Arc::new(reqwest::Client::new()),
        }
    }
}

/// A GitHub repository object as returned by the GitHub API.
///
/// This object is used to represent a GitHub repository and its associated metadata.
/// In its raw JSON form, it looks something like the following:
///
/// ```json
///
/// {
///   "id": 1296269,
///   "node_id": "MDEwOlJlcG9zaXRvcnkxMjk2MjY5",
///   "name": "Hello-World",
///   "full_name": "octocat/Hello-World",
///   "owner": {
///     "login": "octocat",
///     "id": 1,
///     "node_id": "MDQ6VXNlcjE=",
///     "avatar_url": "https://github.com/images/error/octocat_happy.gif",
///     "gravatar_id": "",
///     "url": "https://api.github.com/users/octocat",
///     "html_url": "https://github.com/octocat",
///     "followers_url": "https://api.github.com/users/octocat/followers",
///     "following_url": "https://api.github.com/users/octocat/following{/other_user}",
///     "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
///     "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
///     "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
///     "organizations_url": "https://api.github.com/users/octocat/orgs",
///     "repos_url": "https://api.github.com/users/octocat/repos",
///     "events_url": "https://api.github.com/users/octocat/events{/privacy}",
///     "received_events_url": "https://api.github.com/users/octocat/received_events",
///     "type": "User",
///     "site_admin": false
///   },
///   "private": false,
///   "html_url": "https://github.com/octocat/Hello-World",
///   "description": "This your first repo!",
///   "fork": false,
///   "url": "https://api.github.com/repos/octocat/Hello-World",
///   "archive_url": "https://api.github.com/repos/octocat/Hello-World/{archive_format}{/ref}",
///   "assignees_url": "https://api.github.com/repos/octocat/Hello-World/assignees{/user}",
///   "blobs_url": "https://api.github.com/repos/octocat/Hello-World/git/blobs{/sha}",
///   "branches_url": "https://api.github.com/repos/octocat/Hello-World/branches{/branch}",
///   "collaborators_url": "https://api.github.com/repos/octocat/Hello-World/collaborators{/collaborator}",
///   "comments_url": "https://api.github.com/repos/octocat/Hello-World/comments{/number}",
///   "commits_url": "https://api.github.com/repos/octocat/Hello-World/commits{/sha}",
///   "compare_url": "https://api.github.com/repos/octocat/Hello-World/compare/{base}...{head}",
///   "contents_url": "https://api.github.com/repos/octocat/Hello-World/contents/{+path}",
///   "contributors_url": "https://api.github.com/repos/octocat/Hello-World/contributors",
///   "deployments_url": "https://api.github.com/repos/octocat/Hello-World/deployments",
///   "downloads_url": "https://api.github.com/repos/octocat/Hello-World/downloads",
///   "events_url": "https://api.github.com/repos/octocat/Hello-World/events",
///   "forks_url": "https://api.github.com/repos/octocat/Hello-World/forks",
///   "git_commits_url": "https://api.github.com/repos/octocat/Hello-World/git/commits{/sha}",
///   "git_refs_url": "https://api.github.com/repos/octocat/Hello-World/git/refs{/sha}",
///   "git_tags_url": "https://api.github.com/repos/octocat/Hello-World/git/tags{/sha}",
///   "git_url": "git:github.com/octocat/Hello-World.git",
///   "issue_comment_url": "https://api.github.com/repos/octocat/Hello-World/issues/comments{/number}",
///   "issue_events_url": "https://api.github.com/repos/octocat/Hello-World/issues/events{/number}",
///   "issues_url": "https://api.github.com/repos/octocat/Hello-World/issues{/number}",
///   "keys_url": "https://api.github.com/repos/octocat/Hello-World/keys{/key_id}",
///   "labels_url": "https://api.github.com/repos/octocat/Hello-World/labels{/name}",
///   "languages_url": "https://api.github.com/repos/octocat/Hello-World/languages",
///   "merges_url": "https://api.github.com/repos/octocat/Hello-World/merges",
///   "milestones_url": "https://api.github.com/repos/octocat/Hello-World/milestones{/number}",
///   "notifications_url": "https://api.github.com/repos/octocat/Hello-World/notifications{?since,all,participating}",
///   "pulls_url": "https://api.github.com/repos/octocat/Hello-World/pulls{/number}",
///   "releases_url": "https://api.github.com/repos/octocat/Hello-World/releases{/id}",
///   "ssh_url": "git@github.com:octocat/Hello-World.git",
///   "stargazers_url": "https://api.github.com/repos/octocat/Hello-World/stargazers",
///   "statuses_url": "https://api.github.com/repos/octocat/Hello-World/statuses/{sha}",
///   "subscribers_url": "https://api.github.com/repos/octocat/Hello-World/subscribers",
///   "subscription_url": "https://api.github.com/repos/octocat/Hello-World/subscription",
///   "tags_url": "https://api.github.com/repos/octocat/Hello-World/tags",
///   "teams_url": "https://api.github.com/repos/octocat/Hello-World/teams",
///   "trees_url": "https://api.github.com/repos/octocat/Hello-World/git/trees{/sha}",
///   "clone_url": "https://github.com/octocat/Hello-World.git",
///   "mirror_url": "git:git.example.com/octocat/Hello-World",
///   "hooks_url": "https://api.github.com/repos/octocat/Hello-World/hooks",
///   "svn_url": "https://svn.github.com/octocat/Hello-World",
///   "homepage": "https://github.com",
///   "language": null,
///   "forks_count": 9,
///   "stargazers_count": 80,
///   "watchers_count": 80,
///   "size": 108,
///   "default_branch": "master",
///   "open_issues_count": 0,
///   "is_template": false,
///   "topics": [
///     "octocat",
///     "atom",
///     "electron",
///     "api"
///   ],
///   "has_issues": true,
///   "has_projects": true,
///   "has_wiki": true,
///   "has_pages": false,
///   "has_downloads": true,
///   "has_discussions": false,
///   "archived": false,
///   "disabled": false,
///   "visibility": "public",
///   "pushed_at": "2011-01-26T19:06:43Z",
///   "created_at": "2011-01-26T19:01:12Z",
///   "updated_at": "2011-01-26T19:14:43Z",
///   "permissions": {
///     "admin": false,
///     "push": false,
///     "pull": true
///   },
///   "security_and_analysis": {
///     "advanced_security": {
///       "status": "enabled"
///     },
///     "secret_scanning": {
///       "status": "enabled"
///     },
///     "secret_scanning_push_protection": {
///       "status": "disabled"
///     },
///     "secret_scanning_non_provider_patterns": {
///       "status": "disabled"
///     }
///   }
/// }
/// ```
#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GitHubRepo {
    pub id: u64,
    pub node_id: String,
    pub name: String,
    pub full_name: String,
    pub owner: GitHubUser,
    pub description: Option<String>,
    pub private: bool,
    pub fork: bool,

    pub html_url: String,
    pub url: String,
    pub clone_url: String,
    pub homepage: Option<String>,
    pub language: Option<String>,
    pub forks_count: u64,
    pub stargazers_count: u64,
    pub watchers_count: u64,
    pub size: u64,
    pub default_branch: String,
    pub open_issues_count: u64,
    pub is_template: bool,
    pub topics: Vec<String>,
    pub has_issues: bool,
    pub has_projects: bool,
    pub has_wiki: bool,
    pub has_pages: bool,
    pub has_downloads: bool,
    pub has_discussions: bool,
    pub archived: bool,
    pub disabled: bool,

    pub pushed_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Display for GitHubRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.full_name)
    }
}

impl MetadataSource for GitHubRepo {
    fn inject_metadata(&self, metadata: &mut crate::entities::Metadata) {
        metadata.insert("repo.name", self.name.as_str());
        metadata.insert("repo.fullname", self.full_name.as_str());
        metadata.insert("repo.private", self.private);
        metadata.insert("repo.public", !self.private);
        metadata.insert("repo.fork", self.fork);
        metadata.insert("repo.size", self.size as u32);
        metadata.insert("repo.archived", self.archived);
        metadata.insert("repo.default_branch", self.default_branch.as_str());
        metadata.insert("repo.empty", self.size == 0);
    }
}

/// A user returned by the GitHub API.
///
/// ```json
///   {
///     "login": "octocat",
///     "id": 1,
///     "node_id": "MDQ6VXNlcjE=",
///     "avatar_url": "https://github.com/images/error/octocat_happy.gif",
///     "gravatar_id": "",
///     "url": "https://api.github.com/users/octocat",
///     "html_url": "https://github.com/octocat",
///     "followers_url": "https://api.github.com/users/octocat/followers",
///     "following_url": "https://api.github.com/users/octocat/following{/other_user}",
///     "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
///     "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
///     "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
///     "organizations_url": "https://api.github.com/users/octocat/orgs",
///     "repos_url": "https://api.github.com/users/octocat/repos",
///     "events_url": "https://api.github.com/users/octocat/events{/privacy}",
///     "received_events_url": "https://api.github.com/users/octocat/received_events",
///     "type": "User",
///     "site_admin": false
///   }
/// ```
#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: u64,
    pub node_id: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub site_admin: bool,
}

impl Display for GitHubUser {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.login)
    }
}

/// A release returned by the GitHub API.
///
/// ```json
/// {
///   "url": "https://api.github.com/repos/octocat/Hello-World/releases/1",
///   "html_url": "https://github.com/octocat/Hello-World/releases/v1.0.0",
///   "assets_url": "https://api.github.com/repos/octocat/Hello-World/releases/1/assets",
///   "upload_url": "https://uploads.github.com/repos/octocat/Hello-World/releases/1/assets{?name,label}",
///   "tarball_url": "https://api.github.com/repos/octocat/Hello-World/tarball/v1.0.0",
///   "zipball_url": "https://api.github.com/repos/octocat/Hello-World/zipball/v1.0.0",
///   "id": 1,
///   "node_id": "MDc6UmVsZWFzZTE=",
///   "tag_name": "v1.0.0",
///   "target_commitish": "master",
///   "name": "v1.0.0",
///   "body": "Description of the release",
///   "draft": false,
///   "prerelease": false,
///   "created_at": "2013-02-27T19:35:32Z",
///   "published_at": "2013-02-27T19:35:32Z",
///   "author": {
///     "login": "octocat",
///     "id": 1,
///     "node_id": "MDQ6VXNlcjE=",
///     "avatar_url": "https://github.com/images/error/octocat_happy.gif",
///     "gravatar_id": "",
///     "url": "https://api.github.com/users/octocat",
///     "html_url": "https://github.com/octocat",
///     "followers_url": "https://api.github.com/users/octocat/followers",
///     "following_url": "https://api.github.com/users/octocat/following{/other_user}",
///     "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
///     "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
///     "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
///     "organizations_url": "https://api.github.com/users/octocat/orgs",
///     "repos_url": "https://api.github.com/users/octocat/repos",
///     "events_url": "https://api.github.com/users/octocat/events{/privacy}",
///     "received_events_url": "https://api.github.com/users/octocat/received_events",
///     "type": "User",
///     "site_admin": false
///   },
///   "assets": [
///     {
///       "url": "https://api.github.com/repos/octocat/Hello-World/releases/assets/1",
///       "browser_download_url": "https://github.com/octocat/Hello-World/releases/download/v1.0.0/example.zip",
///       "id": 1,
///       "node_id": "MDEyOlJlbGVhc2VBc3NldDE=",
///       "name": "example.zip",
///       "label": "short description",
///       "state": "uploaded",
///       "content_type": "application/zip",
///       "size": 1024,
///       "download_count": 42,
///       "created_at": "2013-02-27T19:35:32Z",
///       "updated_at": "2013-02-27T19:35:32Z",
///       "uploader": {
///         "login": "octocat",
///         "id": 1,
///         "node_id": "MDQ6VXNlcjE=",
///         "avatar_url": "https://github.com/images/error/octocat_happy.gif",
///         "gravatar_id": "",
///         "url": "https://api.github.com/users/octocat",
///         "html_url": "https://github.com/octocat",
///         "followers_url": "https://api.github.com/users/octocat/followers",
///         "following_url": "https://api.github.com/users/octocat/following{/other_user}",
///         "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
///         "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
///         "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
///         "organizations_url": "https://api.github.com/users/octocat/orgs",
///         "repos_url": "https://api.github.com/users/octocat/repos",
///         "events_url": "https://api.github.com/users/octocat/events{/privacy}",
///         "received_events_url": "https://api.github.com/users/octocat/received_events",
///         "type": "User",
///         "site_admin": false
///       }
///     }
///   ]
/// }
/// ```
#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GitHubRelease {
    pub url: String,
    pub html_url: String,
    pub assets_url: String,
    pub tarball_url: Option<String>,
    pub zipball_url: Option<String>,

    pub id: u64,
    pub node_id: String,
    pub tag_name: String,
    pub target_commitish: String,
    pub name: String,
    pub body: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,

    pub author: GitHubUser,

    pub assets: Vec<GitHubReleaseAsset>,
}

impl MetadataSource for GitHubRelease {
    fn inject_metadata(&self, metadata: &mut crate::entities::Metadata) {
        metadata.insert("release.tag", self.tag_name.as_str());
        metadata.insert("release.name", self.name.as_str());
        metadata.insert("release.draft", self.draft);
        metadata.insert("release.prerelease", self.prerelease);
        metadata.insert("release.published", self.published_at.is_some());
    }
}

/// A release asset returned by the GitHub API.
///
/// ```json
/// {
///   "url": "https://api.github.com/repos/octocat/Hello-World/releases/assets/1",
///   "browser_download_url": "https://github.com/octocat/Hello-World/releases/download/v1.0.0/example.zip",
///   "id": 1,
///   "node_id": "MDEyOlJlbGVhc2VBc3NldDE=",
///   "name": "example.zip",
///   "label": "short description",
///   "state": "uploaded",
///   "content_type": "application/zip",
///   "size": 1024,
///   "download_count": 42,
///   "created_at": "2013-02-27T19:35:32Z",
///   "updated_at": "2013-02-27T19:35:32Z",
///   "uploader": {
///     "login": "octocat",
///     "id": 1,
///     "node_id": "MDQ6VXNlcjE=",
///     "avatar_url": "https://github.com/images/error/octocat_happy.gif",
///     "gravatar_id": "",
///     "url": "https://api.github.com/users/octocat",
///     "html_url": "https://github.com/octocat",
///     "followers_url": "https://api.github.com/users/octocat/followers",
///     "following_url": "https://api.github.com/users/octocat/following{/other_user}",
///     "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
///     "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
///     "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
///     "organizations_url": "https://api.github.com/users/octocat/orgs",
///     "repos_url": "https://api.github.com/users/octocat/repos",
///     "events_url": "https://api.github.com/users/octocat/events{/privacy}",
///     "received_events_url": "https://api.github.com/users/octocat/received_events",
///     "type": "User",
///     "site_admin": false
///   }
/// }
/// ```
#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GitHubReleaseAsset {
    pub url: String,
    pub browser_download_url: String,
    pub id: u64,
    pub node_id: String,
    pub name: String,
    pub label: Option<String>,
    pub state: String,
    pub content_type: String,
    pub size: u64,
    pub download_count: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub uploader: GitHubUser,
}

impl MetadataSource for GitHubReleaseAsset {
    fn inject_metadata(&self, metadata: &mut crate::entities::Metadata) {
        metadata.insert("asset.name", self.name.as_str());
        metadata.insert("asset.size", self.size);
        metadata.insert("asset.downloaded", self.download_count > 0);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;

    use super::*;

    fn load_test_file<T: DeserializeOwned>(name: &str) -> Result<T, Box<dyn std::error::Error>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join(name);
        let json = std::fs::read_to_string(path)?;
        let value = serde_json::from_str(&json)?;
        Ok(value)
    }

    #[test]
    fn test_deserialize_releases() {
        let cases = vec!["github.releases.0.json", "github.releases.1.json"];

        for case in cases {
            let _releases: Vec<GitHubRelease> =
                load_test_file(case).expect("Failed to load test file");
        }
    }
}
