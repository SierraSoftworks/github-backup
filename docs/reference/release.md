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

```json
{
  // Describes the repository from which releases are being sourced
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
  },

  // Describes a specific release associated with a repository
  "release": {
    // The name of the release as it appears in the GitHub UI
    "name": "v1.0.0",
    // The tag name pointing at the commit which generated the release
    "tag": "v1.0.0",
    // Whether the release is a pre-release
    "prerelease": false,
    // Whether the release is a draft (inverse of published)
    "draft": false,
    /// Whether the release has been published yet or not (inverse of draft)
    "published": true
  },

  // Describes a specific artifact which is part of a release
  "asset": {
    // The name of the release asset
    "name": "github-backup-darwin-arm64",
    // The size of the release asset in kilobytes
    "size": 1024,
    // Whether the asset has been downloaded at least once
    "downloaded": true
  }
}
```