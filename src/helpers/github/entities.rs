use std::fmt::Display;

use crate::{FilterValue, entities::MetadataSource};

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
///     "type": "User",
///     "site_admin": false
///   },
///   "private": false,
///   "html_url": "https://github.com/octocat/Hello-World",
///   "description": "This your first repo!",
///   "fork": false,
///   "url": "https://api.github.com/repos/octocat/Hello-World",
///   "clone_url": "https://github.com/octocat/Hello-World.git",
///   "homepage": "https://github.com",
///   "language": null,
///   "forks_count": 9,
///   "stargazers_count": 80,
///   "watchers_count": 80,
///   "size": 108,
///   "default_branch": "master",
///   "open_issues_count": 0,
///   "is_template": false,
///   "topics": ["octocat", "atom", "electron", "api"],
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
///   "updated_at": "2011-01-26T19:14:43Z"
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
///   "assets": []
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
///   "updated_at": "2013-02-27T19:35:32Z"
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
