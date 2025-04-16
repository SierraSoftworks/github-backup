# GitHub Gists
The `github-backup` tool can also be used to back up GitHub Gists.
This is done using the `github/gist` backup type in your configuration file, 
along with an appropriate `from` directive to define the source of the gists
you wish to back up.

::: note
GitHub Gists don't have explicit "names" — instead, they are identified by a unique *gist ID*.
While GitHub displays a filename as the Gist name in the UI, it's actually just the name of the
*first* file in the Gist, which can change over time.

When backing up a Gist, the repository name will be based on the stable gist ID rather than a
filename. This avoids issues where users rename files or add new ones, which could otherwise
result in the same Gist being backed up multiple times under different names.

If you choose to back up GitHub Gists, be aware that the resulting repositories will be named
using their gist ID rather than the more human-readable names shown on the GitHub website.
:::

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
in your filter to determine which gists should be included in your backup. These fields
are accessed using the `gist.<field>` syntax, for example `gist.public` to determine if
the gist is public or not. 

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
| `gist.forks`            | `integer` | The number of times this gist is forked        |
| `gist.file_names`       | `array`   | List of file names in the gist                 |
| `gist.languages`        | `array`   | List of programming languages used in the gist |
| `gist.type`             | `string`  | MIME-Type of content in the gist               |
