# Query Parameters
When retrieving repositories from GitHub, it can be useful to pass query parameters
to the corresponding GitHub API endpoint. These parameters are most commonly used to
perform pre-filtering of repositories prior to GitHub Backup's filtering logic being
invoked.

## Example

```yaml{8-10} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: user
    to: /backups/personal
    credentials: !Token "your_github_pat"
    properties:
      # The query parameters you wish to pass to GitHub's API
      query: "affiliation=owner"
```

## Supported Parameters
Depending on the endpoint being used, different query parameters may be supported. You
can consult the GitHub API documentation for the endpoint you are using to determine
valid parameters and their supported values.

### `from: user`
When using the `from: user` directive, we call the [`GET /user/repos`](GET /user/repos) endpoint. This
endpoint is documented [here](GET /user/repos) and the most commonly used query parameters
include:

 - `affiliation`: Filters repositories based on their affiliation with the user. Possible values are `owner`, `collaborator`, `organization_member`, or a comma-delimited combination of these.
 - `visibility`: Filters repositories based on their visibility. Possible values are `all`, `public`, or `private`.
 - `type`: Filters repositories based on their type. Possible values are `all`, `owner`, `public`, `private`, or `member`.
 - `since`: Filters repositories based on when they were last updated. This is a timestamp in ISO 8601 format.

::: tip
As a general rule, you should not need to provide any of these parameters - with the defaults providing a good starting point for most users.
The sole exception is the `affiliation=owner` parameter which may be useful if you wish to only backup repositories which you directly own.
:::

### `from: users/<username>`
When using the `from: users/<username>` directive, we call the [`GET /users/:username/repos`](GET /users/.../repos)
endpoint. This endpoint is documented [here](GET /users/.../repos) and the most commonly used query parameters
include:

 - `type`: Filters repositories based on their type. Possible values are `all`, `owner`, or `member`.

### `from: orgs/<org>`
When using the `from: orgs/<org>` directive, we call the [`GET /orgs/:org/repos`](GET /orgs/.../repos) endpoint. This
endpoint is documented [here](GET /orgs/.../repos) and the most commonly used query parameters include:

 - `type`: Filters repositories based on their type. Possible values are `all`, `public`, `private`, `forks`, `sources`, or `member`.

[GET /user/repos]: https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#list-repositories-for-the-authenticated-user
[GET /users/.../repos]: https://docs.github.com/en/rest/reference/repos?apiVersion=2022-11-28#list-repositories-for-a-user
[GET /orgs/.../repos]: https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#list-organization-repositories
