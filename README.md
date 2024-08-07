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
    credentials: !UsernamePassword:
      username: "<your username>"
      password: "<your personal access token>"
  - kind: github/repo
    from: "orgs/my-org"
    to: /backups/work
    filter: '!repo.fork && repo.name contains "awesome"'
  - kind: github/release
    from: "orgs/my-org"
    to: /backups/releases
    filter: '!release.prerelease && !asset.source-code'
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

When writing a filter, the goal is to write a logical expression which evaluates to `true` when
you wish to include a repository and `false` when you wish to exclude it. The filter language supports
several operators and properties which can be used to control this process.

Here are some examples of filters you might choose to use:

  - `!repo.fork || !repo.archived || !repo.empty` - Do not include repositories which are forks, archived, or empty.
  - `repo.private` - Only include private repositories in your list.
  - `repo.public && !repo.fork` - Only include public repositories which are not forks.
  - `repo.name contains "awesome"` - Only include repositories which have "awesome" in their name.
  - `(repo.name contains "awesome" || repo.name contains "cool") && !repo.fork` - Only include repositories which have "awesome" or "cool" in their name and are not forks.
  - `!release.prerelease && !asset.source-code` - Only include release artifacts which are not marked as pre-releases and are not source code archives.
  - `repo.name in ["git-tool", "grey"]` - Only include repositories with the names "git-tool" or "grey".
  - `repo.name startswith "sierra"` - Only include repositories with names that start with "sierra" case-insensitive.
  - `repo.name endswith "blue"` - Only include repositories with names that end with "blue" case-insensitive.