use serde::Deserialize;
use tracing_batteries::prelude::*;

/// Configuration for an HTTP-based cron monitoring solution (such as
/// [Sentry Cron Monitors](https://docs.sentry.io/product/crons/) or
/// [healthchecks.io](https://healthchecks.io)).
///
/// Each field holds an optional URL which will be fetched (via an HTTP `GET`
/// request) when the corresponding state is reached during a scheduled backup
/// run. Any field which is left unset is simply skipped, allowing you to report
/// only the states you care about.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
pub struct MonitorConfig {
    /// The URL to fetch when a backup run starts.
    #[serde(default)]
    pub start: Option<String>,

    /// The URL to fetch when a backup run completes successfully.
    #[serde(default)]
    pub success: Option<String>,

    /// The URL to fetch when a backup run completes with one or more errors.
    #[serde(default)]
    pub failure: Option<String>,
}

/// Reports the lifecycle of a backup run to an HTTP-based cron monitoring
/// service by issuing simple `GET` requests to the URLs configured in
/// [`MonitorConfig`].
///
/// Reporting is best-effort: failures to reach the monitoring service are
/// logged but never propagated, ensuring that a flaky monitor can never cause
/// an otherwise healthy backup run to be reported as failed.
pub struct Monitor {
    config: MonitorConfig,
    client: reqwest::Client,
}

impl Monitor {
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Report that a backup run has started.
    pub async fn on_start(&self) {
        self.ping("start", self.config.start.as_deref()).await;
    }

    /// Report that a backup run has completed successfully.
    pub async fn on_success(&self) {
        self.ping("success", self.config.success.as_deref()).await;
    }

    /// Report that a backup run has completed with one or more errors.
    pub async fn on_failure(&self) {
        self.ping("failure", self.config.failure.as_deref()).await;
    }

    #[tracing::instrument(skip(self, url), fields(monitor.state = state))]
    async fn ping(&self, state: &str, url: Option<&str>) {
        let Some(url) = url else {
            return;
        };

        debug!("Reporting '{state}' state to cron monitor.");

        match self
            .client
            .get(url)
            .header("User-Agent", "SierraSoftworks/github-backup")
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                debug!("Successfully reported '{state}' state to cron monitor.");
            }
            Ok(resp) => {
                warn!(
                    "Cron monitor returned HTTP {} when reporting the '{state}' state.",
                    resp.status()
                );
            }
            Err(e) => {
                warn!("Failed to report the '{state}' state to the cron monitor: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn deserialize_empty() {
        let config: MonitorConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(config, MonitorConfig::default());
    }

    #[test]
    fn deserialize_partial() {
        let config: MonitorConfig =
            serde_yaml::from_str("start: https://example.com/start").unwrap();
        assert_eq!(config.start.as_deref(), Some("https://example.com/start"));
        assert_eq!(config.success, None);
        assert_eq!(config.failure, None);
    }

    #[tokio::test]
    async fn reports_each_state() {
        let server = MockServer::start().await;

        for state in ["start", "success", "failure"] {
            Mock::given(method("GET"))
                .and(path(format!("/{state}")))
                .respond_with(ResponseTemplate::new(200))
                .expect(1)
                .mount(&server)
                .await;
        }

        let monitor = Monitor::new(MonitorConfig {
            start: Some(format!("{}/start", server.uri())),
            success: Some(format!("{}/success", server.uri())),
            failure: Some(format!("{}/failure", server.uri())),
        });

        monitor.on_start().await;
        monitor.on_success().await;
        monitor.on_failure().await;

        // `MockServer` verifies the `.expect(1)` expectations when dropped.
    }

    #[tokio::test]
    async fn unconfigured_states_make_no_request() {
        let server = MockServer::start().await;

        // Any request reaching the server would fail the `expect(0)` guard.
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let monitor = Monitor::new(MonitorConfig::default());

        monitor.on_start().await;
        monitor.on_success().await;
        monitor.on_failure().await;
    }

    #[tokio::test]
    async fn error_responses_are_swallowed() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        let monitor = Monitor::new(MonitorConfig {
            success: Some(format!("{}/success", server.uri())),
            ..Default::default()
        });

        // This must not panic even though the monitor returned an error status.
        monitor.on_success().await;
    }
}
