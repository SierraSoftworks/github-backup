---
home: true

actions:
    - text: Get Started
      link: /guide/

features:
    - title: Multiple Sources
      details: |
        Backup repositories and releases from any GitHub user, organization, or repository that you star,
        enabling you to retain a full copy of your important data.

    - title: Advanced Filtering
      details: |
        Describe complex backup policies using an intuitive filtering language with a rich understanding
        of GitHub's metadata.

    - title: Extensive Telemetry
      details: |
        Native support for OpenTelemetry tracing, with the ability to export your trace data to any OTLP
        endpoint for extensive visibility into the health of your backups.
---


This tool is designed to automatically pull the list of GitHub repositories from one, or more,
GitHub organizations and clone (or fetch) them to your local machine. It is designed to be run
as part of a scheduled backup process with the ultimate goal of ensuring that you have a local
copy of all of your GitHub repositories should the unthinkable happen.

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
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: user # The user associated with the provided credentials
    to: /backups/personal
    credentials: !UsernamePassword:
      username: "<your username>"
      password: "<your personal access token>"
    properties:
      query: "affiliation=owner" # Additional query parameters to pass to GitHub when fetching repositories

  - kind: github/repo
    from: "users/another-user"
    to: /backups/friend
    credentials: !Token "your_github_token"

  - kind: github/repo
    from: "orgs/my-org"
    to: /backups/work
    filter: '!repo.fork && repo.name contains "awesome"'

  - kind: github/release
    from: "orgs/my-org"
    to: /backups/releases
    filter: '!release.prerelease && !asset.source-code'

  # You can also backup single repositories directly if you wish
  - kind: github/repo
    from: "repos/my-org/repo"
    to: /backups/work

  # This is particularly useful for backing up release artifacts for
  # specific projects.
  - kind: github/release
    from: "repos/my-org/repo"
    to: /backups/releases
    filter: '!release.prerelease'

  # Backup all repositories starred by the currently authenticated user
  - kind: github/repo
    from: "starred"
    to: /backups/starred/repos
    credentials: !Token "your_github_pat"

  # Backup all GitHub Gists for your authenticated user
  - kind: github/gist
    from: "user"
    to: /backups/gists/user
    credentials: !Token "your_github_token"

  # Backup all Gists starred by the currently authenticated user
  - kind: github/gist
    from: "starred"
    to: /backups/starred/gists
    credentials: !Token "your_github_pat"

  # Backup public GitHub Gist of another user
  - kind: github/gist
    from: "users/another-user"
    to: /backups/gists/another-user
```

<ClientOnly>
    <Contributors repo="SierraSoftworks/github-backup" />
</ClientOnly>
