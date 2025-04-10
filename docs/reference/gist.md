# GitHub Gists
The `github-backup` tool can also be used to back up GitHub Gists.
This is done using the `github/gist` backup type in your configuration file, 
along with an appropriate `from` directive to define the source of the gists
you wish to back up.

## Examples
The following examples show how you can combine the `kind` and `from` directives
to back up gists from GitHub.

::: warning
Not every combination of `kind` and `from` is supported due to limitations in
GitHub's API. If you use an unsupported combination, the tool will fail to
run and provide an error message explaining why.
:::

```yaml{5-6,11-12,16-17,22-23,27-28} title="config.yaml"
schedule: "0 * * * *"

backups:
    # Backup all of the gist accessible to the user associated with the provided credentials
  - kind: github/gist
    from: "user"
    to: /backups/github/gist
    credentials: !Token "your_github_pat"

    # Backup all of the public gist owned by the specified user
  - kind: github/gist
    from: "users/<username>"
    to: /backups/github/gist
```

## Filter Fields
Regardless of which backup kind and source you choose, you may use the following fields
in your filter to determine which repositories should be included in your backup. These fields
are accessed using the `repo.<field>` syntax, for example `repo.fork` to determine if a repository
is a fork.

::: tip
These fields are also available when using [`github/repo`](./repo.md) or [`github/release`](./release.md) backups.
:::


| Field                   | Type      | Description                                    |
|-------------------------|-----------|------------------------------------------------|
| `gist.public`           | `boolean` | Whether the gist is public                     |
| `gist.private`          | `boolean` | Whether the gist is private                    |
| `gist.comments_enabled` | `boolean` | Whether comments are enabled for the gist      |
| `gist.comments`         | `integer` | Number of comments on the gist                 |
| `gist.files`            | `integer` | Number of files in the gist                    |
| `gist.file_names`       | `array`   | List of file names in the gist                 |
| `gist.languages`        | `array`   | List of programming languages used in the gist |
| `gist.type`             | `string`  | Type of content in the gist                    |
