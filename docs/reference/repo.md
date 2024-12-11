# GitHub Repos
The primary use case for `github-backup` is to backup GitHub repositories,
this is done using the `github/repo` or `github/star` backup types in your
configuration file with an appropriate `from` directive to define the source
of the repositories you wish to backup.

## Examples
The following showcases the various combinations of `kind` and `from` directives
you can use to backup repositories from GitHub.

::: warning
Not every combination of `kind` and `from` is supported due to limitations in
the way that GitHub's API works. If you use an unsupported combination, the tool
will fail to run and provide you with an error message explaining why.
:::

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

    # Backup all of the repositories starred by a specific user
  - kind: github/star
    from: "users/<username>"
    to: /backups/github
```

## Filter Fields
Regardless of which backup kind and source you choose, you may use the following fields
in your filter to determine which repositories should be included in your backup. These fields
are accessed using the `repo.<field>` syntax, for example `repo.fork` to determine if a repository
is a fork.

::: tip
These fields are also available when using [`github/release`](./release.md) backups.
:::

```json
{
  "repo": {
    // The name of the repository, excluding its owner
    "name": "Hello-World",
    // The full name of the repository, including its owner
    "fullname": "octocat/Hello-World",
    // Whether the repository is private (inverse of repo.public)
    "private": false,
    // Whether the repository is publicly accessible (inverse of repo.private)
    "public": true,
    // Whether the repository has been forked from another repository.
    "fork": false,
    // The size of the repository in kilobytes, will be zero for empty repositories.
    "size": 1024,
    // Whether the repository has been archived (and is read only).
    "archived": false,
    // Whether the repository has been disabled (and is read only).
    "disabled": false,
    // The name of the main branch for the repository.
    "default_branch": "main",
    // Whether the repository is empty (has a size of 0kB).
    "empty": false,
    // Whether the repository is a template which can be used to create new repositories.
    "template": false,
    // The number of times this repository has been forked.
    "forks": 0,
    // The number of people who have starred this repository.
    "stargazers": 501
  }
}
```
