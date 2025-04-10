# GitHub Releases
In addition to supporting the backup of GitHub repositories, this tool
also allows you to backup release artifacts from your repositories. This
can be useful if you want to ensure that you have a copy of things like
your project's binaries, documentation, or source code archives.

To backup release artifacts, you should use the `github/release` backup
kind in your configuration file. This kind supports the same `from`
directives as the `github/repo` kind, allowing you to backup releases
from your own repositories, those of other users, or those of an organization.

## Examples

```yaml{5-6,11-12,16-17,22-23} title="config.yaml"
schedule: "0 * * * *"

backups:
    # Backup releases from all of the repositories accessible to the user associated with the provided credentials
  - kind: github/release
    from: "user"
    to: /backups/github
    credentials: !Token "your_github_pat"

    # Backup releases from all of the public repositories owned by the specified user
  - kind: github/release
    from: "users/<username>"
    to: /backups/github

    # Backup releases from all of the repositories owned by the specified organization which the provided credentials have access to
  - kind: github/release
    from: "orgs/<org>"
    to: /backups/github
    credentials: !Token "your_github_pat"

    # Backup releases from a specific repository
  - kind: github/release
    from: "repos/<owner>/<repo>"
    to: /backups/github
```

## Filter Fields
When backing up release artifacts, you may use the following fields in your filter
expressions. These fields are accessed using the `release.<field>` syntax, for example
`release.prerelease` to determine if a release is a pre-release.

For `kind: github/release`

| Field                 | Type       | Description (_Example_)                                            |
|-----------------------|------------|--------------------------------------------------------------------|
| `release.tag`         | `string`   | The name of the tag (_v1.0.0_)                                     |
| `release.name`        | `string`   | The name of the release (_v1.0.0_)                                 |
| `release.draft`       | `boolean`  | Whether the release is a draft (unpublished) release               |
| `release.prerelease`  | `boolean`  | Whether to identify the release as a prerelease or a full release  |
| `release.published`   | `boolean`  | Whether the release is a published (not a draft) release           |
| `asset.name`          | `string`   | The file name of the asset (_github-backup-darwin-arm64_)          |
| `asset.size`          | `integer`  | The size of the asset, in kilobytes. (_1024_)                      |
| `asset.downloaded`    | `boolean`  | If the asset was downloaded at least once from the GitHub Release  |

