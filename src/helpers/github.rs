use human_errors::ResultExt;
use reqwest::{Method, StatusCode, Url, header::LINK};
use std::{
    fmt::Display,
    sync::{Arc, atomic::AtomicBool},
};
use tokio_stream::Stream;

use crate::{
    FilterValue,
    entities::{Credentials, MetadataSource},
    errors::{HumanizableError as _, ResponseError},
};

#[derive(Clone)]
pub struct GitHubClient {
    client: Arc<reqwest::Client>,

    #[cfg(test)]
    mock_replies: std::collections::HashMap<String, MockResponse>,
}

impl GitHubClient {
    #[allow(dead_code)]
    pub async fn get<U: AsRef<str>, T: serde::de::DeserializeOwned>(
        &self,
        url: U,
        creds: &Credentials,
        cancel: &AtomicBool,
    ) -> Result<T, human_errors::Error> {
        let resp = self.call(Method::GET, &url, creds, |r| r, cancel).await?;

        resp.json().await.map_err(|e| {
            human_errors::wrap_system(
                e,
                format!(
                    "Unable to parse GitHub's response for '{}' due to invalid JSON.",
                    url.as_ref()
                ),
                &["Please report this issue to us on GitHub."],
            )
        })
    }

    pub fn get_paginated<'a, U: AsRef<str> + 'a, T: serde::de::DeserializeOwned + 'a>(
        &'a self,
        page_url: U,
        creds: &'a Credentials,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<T, human_errors::Error>> + 'a {
        async_stream::try_stream! {
          let mut page_url = Some(page_url.as_ref().to_string());

          while let Some(url) = page_url {
              if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  Err(human_errors::user(
                      "The backup operation was cancelled by the user. Only partial data may have been backed up.",
                      &["Allow the backup to complete fully before cancelling again."]))?;
              }

              let resp = self.call(Method::GET, &url, creds, |r| r, cancel).await?;

              if let Some(link_header) = resp.headers().get(LINK) {
                  let link_header = link_header.to_str().wrap_err_as_system(
                      "Unable to parse GitHub's Link header due to invalid characters, which will result in pagination failing to work correctly.",
                      &["Please report this issue to us on GitHub."])?;

                  let links = parse_link_header::parse_with_rel(link_header).wrap_err_as_system(
                    "Unable to parse GitHub's Link header, which will result in pagination failing to work correctly.",
                    &["Please report this issue to us on GitHub."])?;

                  if let Some(next_link) = links.get("next") {
                      page_url = Some(next_link.raw_uri.to_string());
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
                  Err(human_errors::wrap_system(
                    err,
                    format!("Unable to parse GitHub response into the expected structure when requesting '{}'.", &url),
                    &["Please report this issue to us on GitHub."],
                ))?;
                }
              }
          }
        }
    }

    async fn call<U: AsRef<str>, B>(
        &self,
        method: Method,
        url: U,
        creds: &Credentials,
        builder: B,
        _cancel: &AtomicBool,
    ) -> Result<reqwest::Response, human_errors::Error>
    where
        B: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        let parsed_url: Url = url.as_ref().parse().wrap_err_as_user(
            format!(
                "Unable to parse GitHub URL '{}' as a valid URL.",
                url.as_ref()
            ),
            &["Make sure that you have configured your GitHub API correctly."],
        )?;

        #[cfg(test)]
        if let Some(response) = self.mock_replies.get(parsed_url.path()) {
            return Ok(response.into());
        } else if !self.mock_replies.is_empty() {
            panic!(
                "No mock response found for '{}'. Available mocks: {:?}",
                parsed_url.path(),
                self.mock_replies.keys()
            );
        }

        let mut req = self
            .client
            .request(method, parsed_url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "SierraSoftworks/github-backup");

        req = match creds {
            Credentials::None => req,
            Credentials::Token(token) => req.bearer_auth(token),
            Credentials::UsernamePassword { username, password } => {
                req.basic_auth(username, Some(password))
            }
        };

        let req = builder(req);

        let resp = req.send().await.map_err(|e| e.to_human_error())?;

        if resp.status().is_success() {
            Ok(resp)
        } else if resp.status() == StatusCode::UNAUTHORIZED {
            Err(human_errors::user(
                "The access token you have provided was rejected by the GitHub API.",
                &["Make sure that your GitHub token is valid and has not expired."],
            ))
        } else {
            let err = ResponseError::with_body(resp).await;
            let status = err.status_code;
            Err(human_errors::wrap_user(
                err,
                format!("The GitHub API returned an error response with status code {status}."),
                &["Please check the error message below and try again."],
            ))
        }
    }

    #[cfg(test)]
    pub fn mock<B: FnOnce(MockResponse) -> MockResponse>(mut self, path: &str, builder: B) -> Self {
        self.mock_replies
            .insert(path.to_string(), builder(MockResponse::new(StatusCode::OK)));
        self
    }
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self {
            client: Arc::new(reqwest::Client::new()),

            #[cfg(test)]
            mock_replies: std::collections::HashMap::new(),
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
        metadata.insert("repo.disabled", self.disabled);
        metadata.insert("repo.default_branch", self.default_branch.as_str());
        metadata.insert("repo.empty", self.size == 0);
        metadata.insert("repo.template", self.is_template);
        metadata.insert("repo.forks", self.forks_count as u32);
        metadata.insert("repo.stargazers", self.stargazers_count as u32);
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

/// A GitHub gist object as returned by the GitHub API.
///
/// This object is used to represent a GitHub gist and its associated metadata.
/// In its raw JSON form, it looks something like the following:
///
/// ```json
/// {
///   "url": "https://api.github.com/gists/58722b4c6488ceefe0207629acad53bc",
///   "forks_url": "https://api.github.com/gists/58722b4c6488ceefe0207629acad53bc/forks",
///   "commits_url": "https://api.github.com/gists/58722b4c6488ceefe0207629acad53bc/commits",
///   "id": "58722b4c6488ceefe0207629acad53bc",
///   "node_id": "G_kwDOAB3LV9oAIDU4NzIyYjRjNjQ4OGNlZWZlMDIwNzYyOWFjYWQ1M2Jj",
///   "git_pull_url": "https://gist.github.com/58722b4c6488ceefe0207629acad53bc.git",
///   "git_push_url": "https://gist.github.com/58722b4c6488ceefe0207629acad53bc.git",
///   "html_url": "https://gist.github.com/cedi/58722b4c6488ceefe0207629acad53bc",
///   "files": {
///     "Lifecycle management system for an HPC cluster.md": {
///       "filename": "Lifecycle management system for an HPC cluster.md",
///       "type": "text/markdown",
///       "language": "Markdown",
///       "raw_url": "https://gist.githubusercontent.com/cedi/58722b4c6488ceefe0207629acad53bc/raw/625e2cb5b7bcf8fc7dd31b74d3ace11b011c67c0/Lifecycle%20management%20system%20for%20an%20HPC%20cluster.md",
///       "size": 23984
///     }
///   },
///   "public": false,
///   "created_at": "2025-04-05T11:31:01Z",
///   "updated_at": "2025-04-06T18:34:00Z",
///   "description": "",
///   "comments": 0,
///   "user": null,
///   "comments_enabled": true,
///   "comments_url": "https://api.github.com/gists/58722b4c6488ceefe0207629acad53bc/comments",
///   "owner": {
///     "login": "cedi",
///     "id": 1952599,
///     "node_id": "MDQ6VXNlcjE5NTI1OTk=",
///     "avatar_url": "https://avatars.githubusercontent.com/u/1952599?v=4",
///     "gravatar_id": "",
///     "url": "https://api.github.com/users/cedi",
///     "html_url": "https://github.com/cedi",
///     "followers_url": "https://api.github.com/users/cedi/followers",
///     "following_url": "https://api.github.com/users/cedi/following{/other_user}",
///     "gists_url": "https://api.github.com/users/cedi/gists{/gist_id}",
///     "starred_url": "https://api.github.com/users/cedi/starred{/owner}{/repo}",
///     "subscriptions_url": "https://api.github.com/users/cedi/subscriptions",
///     "organizations_url": "https://api.github.com/users/cedi/orgs",
///     "repos_url": "https://api.github.com/users/cedi/repos",
///     "events_url": "https://api.github.com/users/cedi/events{/privacy}",
///     "received_events_url": "https://api.github.com/users/cedi/received_events",
///     "type": "User",
///     "user_view_type": "public",
///     "site_admin": false
///   },
///   "truncated": false
/// }
/// ```
#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GitHubGist {
    pub id: String,
    pub node_id: String,
    pub owner: Option<GitHubUser>,
    pub description: Option<String>,
    pub public: bool,

    pub url: String,
    pub forks_url: String,
    pub commits_url: String,
    pub git_pull_url: String,
    pub git_push_url: String,
    pub html_url: String,
    pub comments: u64,
    pub user: Option<serde_json::Value>, // `null` in the example
    pub comments_enabled: Option<bool>,
    pub comments_url: String,
    pub truncated: bool,
    pub files: std::collections::HashMap<String, GistFile>,
    pub forks: Option<Vec<serde_json::Value>>,
    pub history: Option<Vec<serde_json::Value>>,

    pub created_at: String,
    pub updated_at: String,
}

impl MetadataSource for GitHubGist {
    fn inject_metadata(&self, metadata: &mut crate::entities::Metadata) {
        metadata.insert("gist.public", self.public);
        metadata.insert("gist.private", !self.public);
        metadata.insert("gist.comments_enabled", self.comments_enabled);
        metadata.insert("gist.comments", self.comments);
        metadata.insert("gist.files", self.files.len() as u32);
        metadata.insert("gist.forks", self.forks.iter().count() as u32);
        metadata.insert(
            "gist.file_names",
            self.files
                .keys()
                .map(|k| FilterValue::from(k.as_str()))
                .collect::<Vec<FilterValue>>(),
        );
        metadata.insert(
            "gist.languages",
            self.files
                .values()
                .filter_map(|file| file.language.as_deref()) // gets &str from Option<String>
                .map(FilterValue::from)
                .collect::<Vec<FilterValue>>(),
        );

        metadata.insert(
            "gist.type",
            self.files
                .values()
                .map(|file| FilterValue::from(file.type_.as_str()))
                .collect::<Vec<FilterValue>>(),
        );
    }
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GistFile {
    pub filename: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub language: Option<String>,
    pub raw_url: String,
    pub size: u64,
}

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

#[cfg(test)]
#[derive(Clone)]
pub struct MockResponse {
    pub status: StatusCode,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<String>,
}

#[cfg(test)]
impl MockResponse {
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            headers: std::collections::HashMap::new(),
            body: None,
        }
    }

    pub fn with_status_code<S: Into<StatusCode>>(mut self, status: S) -> Self {
        self.status = status.into();
        self
    }

    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_body<B: Into<String>>(mut self, body: B) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn with_body_from_file(mut self, name: &str) -> Self {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join(name);

        let json = std::fs::read_to_string(path).expect("Failed to read test file");

        self.body = Some(json);
        self
    }
}

#[cfg(test)]
impl From<&MockResponse> for reqwest::Response {
    fn from(mock: &MockResponse) -> reqwest::Response {
        let mut builder = http::Response::builder().status(mock.status);

        for (key, value) in mock.headers.iter() {
            builder = builder.header(key, value);
        }

        if let Some(body) = mock.body.as_ref() {
            builder.body(body.clone()).unwrap().into()
        } else {
            builder.body("").unwrap().into()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
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
            let mut metadata = crate::entities::Metadata::default();
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
            let mut metadata = crate::entities::Metadata::default();
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
            let mut metadata = crate::entities::Metadata::default();
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
