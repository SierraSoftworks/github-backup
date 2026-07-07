# Automatic Recovery
When repeatedly backing up repositories over long periods of time, the local copy of a
repository can occasionally end up in a broken state. The most common cause is a backup
run which is interrupted part-way through (e.g. by a power failure or the process being
killed), leaving behind git lock files which block every subsequent update with errors
like this one:

```
Unable to fetch from remote git repository 'https://github.com/example/repo.git' (User error)

This was caused by:
 - Failed to update references to their new position to match their remote locations
 - The lock for the packed-ref file could not be obtained
 - The lock for resource '/backups/repos/example/repo/.git/packed-refs' could not be obtained after 1.00s after 15 attempt(s).
   The lockfile at '/backups/repos/example/repo/.git/packed-refs.lock' might need manual deletion.
```

To avoid the need for manual intervention in these situations, `github-backup` will
automatically attempt to recover repositories which fail to update. How far it is
willing to go is controlled by the `recovery` property on your backup policy.

## Recovery Modes

| Mode              | Behaviour                                                                                                                                                              |
|-------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `none`            | Never attempt automatic recovery, report the error and leave the local repository untouched.                                                                            |
| `non-destructive` | Remove stale git lock files (older than 15 minutes) and retry the fetch. This is the **default**.                                                                       |
| `destructive`     | Everything `non-destructive` does and, if that fails, clone a fresh copy of the repository into a temporary directory and replace the local copy with it if successful. |

```yaml{7-8} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: "repos/my-org/repo"
    to: /backups/work
    properties:
      recovery: destructive
```

## How Destructive Recovery Works
When `recovery: destructive` is configured and a repository cannot be updated (even
after stale locks have been cleaned up), the engine will:

1. Clone a fresh copy of the repository into a hidden staging directory alongside the
   existing backup (e.g. `.repo.recovery`).
2. If (and only if) the clone succeeds, move the existing backup out of the way, move
   the fresh clone into its place, and then remove the old copy.
3. If the clone fails, the existing backup is left completely untouched and the
   original error is reported.

Using a successful clone as the gate means that transient problems — an unreachable
remote, invalid credentials, or a full disk — will never cause your existing (possibly
still valuable) local data to be discarded. The local copy is only ever replaced by a
clone which is known to be complete and healthy.

::: warning
Destructive recovery replaces the local copy of the repository with the remote's
current state. Any data which only existed in your local backup — for example refs
which were force-pushed over or deleted on the remote since the last successful
backup — will be lost. If retaining such history is important to you, keep the
default `non-destructive` mode and resolve persistent corruption manually.
:::
