# GitHub Enterprise
If you run your own GitHub Enterprise instance, you will need to provide the URL
of your instance's API endpoint to GitHub Backup. This is done through the `api_url`
property in your configuration file.

```yaml{8-9} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: user
    to: /backups/github
    credentials: !Token "your_github_pat"
    properties:
      api_url: "https://github.example.com/api/v3"
```

::: tip
You may use a different `api_url` for each backup policy in your configuration file
in scenarios where you run multiple GitHub Enterprise instances.
:::
