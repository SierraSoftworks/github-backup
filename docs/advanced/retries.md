# Retries
When backing up large numbers of repositories, releases, and gists, it is normal to
occasionally hit a transient failure: a network connection is dropped or reset, a remote
server is momentarily overloaded, or a request times out. These failures usually clear up
on their own, so `github-backup` will automatically retry an individual target which fails
before reporting the error.

How many times a failed target is retried is controlled by the `retries` property on your
backup policy. By default each target is retried **once** (for a total of two attempts)
before the failure is reported.

```yaml{7-8} title="config.yaml"
schedule: "0 * * * *"

backups:
  - kind: github/repo
    from: "repos/my-org/repo"
    to: /backups/work
    properties:
      retries: "3"
```

| Value       | Behaviour                                                                                     |
|-------------|-----------------------------------------------------------------------------------------------|
| `0`         | Disable retries; the first failure of a target is reported immediately.                        |
| `1`         | Retry a failed target once before reporting the error. This is the **default**.                |
| `n`         | Retry a failed target up to `n` times (for a total of `n + 1` attempts) before giving up.      |

## How Retries Work
Retries apply to each individual target independently. When a source entity (such as a
repository) is mirrored to several targets, each target is retried on its own, so a
transient failure writing to one destination will not affect the others.

Retries wrap the entire backup of a target, which means they compose with the git
[automatic recovery](./recovery.md) behaviour: each attempt performs its own recovery
steps, and only once every attempt has been exhausted is the failure reported.

::: tip
Retries smooth over transient problems, but they will not paper over a persistent
misconfiguration such as an invalid access token or an unreachable remote — those failures
will simply be retried the configured number of times and then reported as usual. If you
are seeing repeated failures, check the reported error rather than increasing the retry
count.
:::
