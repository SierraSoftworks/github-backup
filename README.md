# GitHub Backup
**Automatically backup your GitHub repositories to your local machine.**

This tool is designed to automatically pull the list of GitHub repositories from one, or more,
GitHub organizations and clone (or fetch) them to your local machine. It is designed to be run
as part of a scheduled backup process with the ultimate goal of ensuring that you have a local
copy of all of your GitHub repositories should the unthinkable happen.

## Features
- **Backup Multiple Organizations**, automatically gathering the full list of repositories for
  each organization through the GitHub API.
- **Repo Allowlists/Denylists** to provide fine-grained control over which repositories are backed
  up and which are not.
- **GitHub Enterprise Support** for those of you running your own GitHub instances and not relying
  on GitHub.com.