# Introduction
At Sierra Softworks we have well over 300 GitHub repositories, some of which are
private, some of which are forks, and some of which are hosted on personal GitHub
accounts. Being able to back these up automatically, with minimal configuration
and solid visibility into the health of our backups is a critical part of our
disaster recovery strategy.

While it is entirely possible to use [Gitea](https://gitea.io) or [GitLab](https://gitlab.com)
to mirror our repositories, we found that doing so required extensive manual maintenance
both of these hosting platforms as well as the creation of mirrors when new repositories
were created.

To solve this, we created GitHub Backup - a lightweight tool which runs a scheduled
backup/sync of your GitHub repositories with advanced filtering support to ensure that
only the repositories you care about are backed up.

## Example
To run GitHub Backup, you will need to download the latest release from the
[GitHub Releases](https://github.com/SierraSoftworks/github-backup/releases)
page. Once you have the binary, you can run it with a configuration file like the one
shown below.

```bash
# Run the tool directly
./github-backup --config config.yaml

# Or run it in a container
docker run \
  -v $(pwd)/config.yaml:/config.yaml \
  -v $(pwd)/backups:/backups \
  ghcr.io/SierraSoftworks/github-backup:latest \
    --config /config.yaml
```

### Configuration

```yaml title="config.yaml"
# Run a backup every hour (will use `git fetch` for existing copies)
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: user
    to: /backups/github
    # Generate a PAT at https://github.com/settings/tokens (with `repo` scope if you want to backup private repositories)
    credentials: !Token "your_github_pat"
    properties:
       # Only backup repositories which we are the owner of
      query: "affiliation=owner"
```

## Scheduling
GitHub Backup is designed to run automatically on a schedule, and it uses the [cron](https://en.wikipedia.org/wiki/Cron)
syntax to determine when it should run. The example above will run the backup every hour, on the hour - but you can
easily configure it to run at any interval you like.

```yaml{2} title="config.yaml"
# Run every 6 hours on weekdays
schedule: "0 */6 * * 1-5"

# Run every day at 3am
schedule: "0 3 * * *"

# Run at 2am on the first day of every month
schedule: "0 2 1 * *"
```

::: tip
You can use [crontab.guru](https://crontab.guru/) to help you configure a cron expression which meets your needs.
:::

## Authentication
GitHub commonly allows free and unauthenticated access to public repositories, however unauthenticated
users have strict rate limits applied to their use of the GitHub API and even the rate at which they
can clone repositories. To avoid these rate limits and access private repositories, you will need to
provide authentication credentials to GitHub Backup.

The most common way to do this is through a [Personal Access Token][github-pat]. GitHub Backup supports
both Classic and Fine Grained PATs, with the latter being the recommended approach as it allows you to
restrict the permissions granted to the token to only those which are required for the backup process.

You can generate a new PAT by visiting the [GitHub Settings](https://github.com/settings/tokens?type=beta)
and clicking the **Generate new token** button. After providing a name and selecting an expiration window,
you can choose which repositories the token should have access to, grant the following permissions to the
token, and then click the **Generate token** button.

### Required Permissions
 * **Repository Permissions &rarr; Contents: Read-only**

   Allows GitHub Backup to download the contents of your repositories and their release artifacts.

 * **Account Permissions &rarr; Starring: Read-only**

   Allows GitHub Backup to determine which repositories you have starred (only required if you use the `from: user/<username>/stars` source type).

Once you have generated your token, you can use it in your configuration file as shown below.

```yaml{7} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: "user"
    to: /backups/github
    credentials: !Token "your_github_pat"
```

## Sources
While backing up your own personal repositories is a great start, you may also have organizational
repositories which you would like to backup. GitHub Backup supports backing up repositories from
a range of different sources which are specified in the `from` directive.

```yaml{5-6,11-12,16-17,22-23,27-28} title="config.yaml"
schedule: "0 * * * *"

backups:
    # Backup all of the repositories accessible to the user associated with the provided credentials
  - kind: github/repo
    from: "user"
    to: /backups/github
    credentials: !Token "your_github_pat"

    # Backup all of the public repositories owned by the specified user
  - kind: github/repo
    from: "users/<username>"
    to: /backups/github

    # Backup all of the repositories owned by the specified organization which the provided credentials have access to
  - kind: github/repo
    from: "orgs/<org>"
    to: /backups/github
    credentials: !Token "your_github_pat"

    # Backup a specific repository
  - kind: github/repo
    from: "repos/<owner>/<repo>"
    to: /backups/github

    # Backup all of the repositories starred by the currently authenticated user
  - kind: github/repo
    from: "starred"
    to: /backups/starred/repos
    credentials: !Token "your_github_pat"
    
    # Backup all GitHub Gist accessible to the user associated with the provided credentials
  - kind: github/gist
    from: "users"
    to: /backups/gists/user
    credentials: !Token "your_github_pat"
    
    # Backup all of the gists starred by the currently authenticated user
  - kind: github/repo
    from: "starred"
    to: /backups/starred/gists
    credentials: !Token "your_github_pat"
        
    # Backup all of the public GitHub Gist by a specific user
  - kind: github/gist
    from: "users/<username>"
    to: /backups/gists/<username>    
```

## Filtering
Of course, you might not want to backup every repository you have access to. To help
with this, GitHub Backup supports a filtering language which allows you to describe
which repositories you want to backup in a more granular way.

For example, the following will only backup repositories whose names contain the word
"awesome" and which are not forks of other repositories.

You can read more about filtering in the [filtering guide](../advanced/filters.md).

```yaml{7} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: "orgs/my-org"
    to: /backups/github
    filter: '!repo.fork && repo.name contains "awesome"'
```

[github-pat]: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens
