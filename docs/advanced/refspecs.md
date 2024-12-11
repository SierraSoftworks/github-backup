# Refspecs
In some cases you may wish to limit which git refs are backed up from a repository.
This can be done by configuring the corresponding `refspecs` field in your backup properties
based on the [Git Refspec](https://git-scm.com/book/en/v2/Git-Internals-The-Refspec) syntax.

For example, to only backup the `main` and `develop` branches from a repository, you would use the following configuration:

```yaml{7-8} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: "repos/my-org/repo"
    to: /backups/work
    properties:
      refspecs: "+refs/heads/main:refs/remotes/origin/main,+refs/heads/develop:refs/remotes/origin/develop"
```

The default `refspecs` configuration is `+refs/heads/*:refs/remotes/origin/*` which will backup all branches from the repository
and automatically update local copies in cases where the remote is force-pushed (i.e. not fast-forward updatable).
