use human_errors::ResultExt;
use reqwest::{Method, StatusCode, Url, header::LINK};
use std::sync::{Arc, atomic::AtomicBool};
use tokio_stream::Stream;

use crate::{
    entities::Credentials,
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
                  let link_header = link_header.to_str().wrap_system_err(
                      "Unable to parse GitHub's Link header due to invalid characters, which will result in pagination failing to work correctly.",
                      &["Please report this issue to us on GitHub."])?;

                  let links = parse_link_header::parse_with_rel(link_header).wrap_system_err(
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
        let parsed_url: Url = url.as_ref().parse().wrap_user_err(
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
