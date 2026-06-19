# Cron Monitoring
GitHub Backup is designed to run unattended on a schedule, which makes it
important to know when a backup run fails to start or complete. To support this,
GitHub Backup can report the state of each scheduled run to an HTTP-based cron
monitoring service such as [Sentry Cron Monitors](https://docs.sentry.io/product/crons/)
or [healthchecks.io](https://healthchecks.io/).

Whenever a backup run starts or completes, GitHub Backup will make a simple HTTP
`GET` request to the URL you've configured for that state, allowing your
monitoring service to track whether your backups are running as expected and to
alert you if they stop.

## Configuration
Monitoring is configured under the top-level `monitor` key in your configuration
file. You may provide a separate URL for each of the `start`, `success`, and
`failure` states, and any state you leave out is simply not reported.

```yaml
schedule: "0 * * * *"

monitor:
  # Fetched when a backup run starts.
  start: https://example.com/monitor/start
  # Fetched when a backup run completes successfully.
  success: https://example.com/monitor/success
  # Fetched when a backup run completes with one or more errors.
  failure: https://example.com/monitor/failure

backups:
  - kind: github/repo
    from: user
    to: /backup/github
    credentials: !Token your_access_token
```

A run is reported as a `failure` if any backup policy reports one or more errors,
and as a `success` otherwise.

::: tip
Reporting is best-effort. If the monitoring service can't be reached, a warning is
logged but the backup run itself is never affected, ensuring that a flaky monitor
can't cause an otherwise healthy backup to be reported as failed.
:::

## Examples

### Sentry
[Sentry's Cron Monitors](https://docs.sentry.io/product/crons/getting-started/http/)
expose a check-in URL which accepts a `status` query parameter. You can point each
state at the same URL while varying the `status` value to report the lifecycle of
your backups.

```yaml
monitor:
  start: https://sentry.io/api/0/organizations/your-org/monitors/github-backup/checkins/?status=in_progress
  success: https://sentry.io/api/0/organizations/your-org/monitors/github-backup/checkins/?status=ok
  failure: https://sentry.io/api/0/organizations/your-org/monitors/github-backup/checkins/?status=error
```

### healthchecks.io
[healthchecks.io](https://healthchecks.io/) provides a base ping URL, with
`/start` and `/fail` suffixes used to signal the start and failure of a run.

```yaml
monitor:
  start: https://hc-ping.com/your-uuid/start
  success: https://hc-ping.com/your-uuid
  failure: https://hc-ping.com/your-uuid/fail
```
