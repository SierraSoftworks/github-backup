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
backup_path: "/backups" # Where to store the backups

# Run a backup every hour (will use `git fetch` for existing copies)
# You can also omit this if you want to run a one-shot backup
schedule: "0 * * * *"
github:
  token: "<your-github-token>" # Optional if you are only backing up public repositories

backups:
  - user: "my-user"
  - org: "my-org"
    filters:
      - !Include ["my-repo-1", "my-repo-2"]
      - !NonFork
```

## Filters
This tool allows you to configure filters to control which GitHub repositories are backed up and
which are not. Filters are used within the `backups` section of your configuration file and can
be specified on a per-user or per-organization basis.

### `!Include [repo1, repo2, ...]`
Only include the specified repositories in the backup. This filter matches names case-insensitively.

### `!Exclude [repo1, repo2, ...]`
Exclude the specified repositories from the backup. This filter matches names case-insensitively.

### `!Fork`
Only include repositories which are forks of other repositories.

### `!NonFork`
Only include repositories which are not forks of other repositories.

### `!Private`
Only include private repositories.

### `!Public`
Only include public repositories.

### `!Archived`
Only include archived repositories.

### `!NonArchived`
Only include repositories which are not archived.