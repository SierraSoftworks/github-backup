# GitHub Backup
**Automatically backup your GitHub repositories to your local machine.**

This tool is designed to automatically pull the list of GitHub repositories from one, or more,
GitHub organizations and clone (or fetch) them to your local machine. It is designed to be run
as part of a scheduled backup process with the ultimate goal of ensuring that you have a local
copy of all of your GitHub repositories should the unthinkable happen.

## Features
- **Backup Multiple Organizations**, automatically gathering the full list of repositories for
  each organization through the GitHub API.
- **Repo Allowlists/Denylists** to provide fine-grained control over which repositories are backed
  up and which are not.
- **GitHub Enterprise Support** for those of you running your own GitHub instances and not relying
  on GitHub.com.

## Example

```bash
# Run the tool directly
./github-backup --config config.yaml

# Or run it in a container
docker run \
  -v $(pwd)/config.yaml:/config.yaml \
  -v $(pwd)/backups:/backups \
  ghcr.io/SierraSoftworks/github-backup:main \
    --config /config.yaml
```

### Configuration

```yaml
# Run a backup every hour (will use `git fetch` for existing copies)
# You can also omit this if you want to run a one-shot backup
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: users/my-user
    to: /backups/personal
    credentials: !Token "<my-user-token>"
  - kind: github/repo
    from: "orgs/my-org"
    to: /backups/work
    filters:
      - !Include ["my-repo-1", "my-repo-2"]
      - !NonFork
  - kind: github/release
    from: "orgs/my-org"
    to: /backups/releases
    filters:
      - !IsNot "prerelease"
      - !IsNot "source-code"
```

### OpenTelemetry Reporting
In addition to the standard logging output, this tool also supports reporting metrics to an
OpenTelemetry-compatible backend. This can be useful for tracking the performance of the tool
over time and configuring monitoring in case backups start to fail.

Configuration is conducted through the use of environment variables:

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=https://your-otel-collector:4317
OTEL_EXPORTER_OTLP_HEADERS=X-API-KEY=your-api-key
OTEL_TRACES_SAMPLER=traceidratio
OTEL_TRACES_SAMPLER_ARG=1.0
```

## Filters
This tool allows you to configure filters to control which GitHub repositories are backed up and
which are not. Filters are used within the `backups` section of your configuration file and can
be specified on a per-user or per-organization basis.

### `!Include [repo1, repo2, ...]`
Only include the specified repositories in the backup. This filter matches names case-insensitively.

### `!Exclude [repo1, repo2, ...]`
Exclude the specified repositories from the backup. This filter matches names case-insensitively.

### `!Is "tag"`
Only include repositories which are tagged with the corresponding tag.

### `!IsNot "tag"`
Only include repositories which are not tagged with the corresponding tag.

## Tags
We support multiple tags which indicate the state of repositories, these include:

- `public` which indicates that a repository is publicly accessible.
- `private` which indicates that a repository is only accessible to authenticated users
  (this is the opposite of `public`).
- `fork` which indicates that a repository has been forked from an upstream repo.
- `archived` which indicates that a repository has been archived and is no longer editable.
- `empty` which indicates that a repository does not have any commits or other data.
