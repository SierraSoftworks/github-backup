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
These fields are also available when using [`github/release`](./release.md) or [`github/gist`](./gist.md) backups.
:::


| Field                  | Type       | Description (_Example_)                                                                            |
|------------------------|------------|----------------------------------------------------------------------------------------------------|
| `repo.name`            | `string`   | The name of the repository (_Hello-World_)                                                         |
| `repo.fullname`        | `string`   | The full-name of the repository (_octocat/Hello-World_)                                            |
| `repo.private`         | `boolean`  | Whether the repository is private                                                                  |
| `repo.public`          | `boolean`  | Whether the repository is public                                                                   |
| `repo.fork`            | `boolean`  | Whether the repository is a fork                                                                   |
| `repo.size`            | `integer`  | The size of the repository, in kilobytes (_1024_).                                                 |
| `repo.archived`        | `boolean`  | Whether the repository is archived                                                                 |
| `repo.disabled`        | `boolean`  | Returns whether or not this repository disabled                                                    |
| `repo.default_branch`  | `string`   | The default branch of the repository (_main_)                                                      |
| `repo.empty`           | `boolean`  | Whether the repository is empty (When a repository is initially created, `repo.empty` is `true`)   |
| `repo.template`        | `boolean`  | Whether this repository acts as a template that can be used to generate new repositories           |
| `repo.forks`           | `integer`  | The number of times this repository is forked                                                      |
| `repo.stargazers`      | `integer`  | The number of people starred this repository                                                       |

