use clap::Parser;
use errors::Error;
use github::GitHubRepo;
use tokio::task::JoinSet;

mod config;
mod errors;
mod git;
mod github;

/// Backup your GitHub repositories automatically.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the configuration file.
    #[arg(short, long, default_value = "config.yaml")]
    pub config: String,

    /// Run in dry-run mode.
    #[arg(short, long)]
    pub dry_run: bool,
}

async fn run(args: Args) -> Result<(), Error> {
    let config = config::Config::try_from(&args)?;

    let github = github::GitHubClient::from(&config);
    let git_backup = git::GitBackupAgent::from(&config);

    for policy in config.backups {
        println!("Backing up repositories for org: {}", policy.org);
        let repos = github.get_repos(&policy.org).await?;
        
        let mut join_set: JoinSet<Result<(GitHubRepo, String), (GitHubRepo, errors::Error)>> = JoinSet::new();
        for repo in repos {
            if policy.filters.iter().all(|p| repo.matches(p)) {
                if args.dry_run {
                    println!(" - {} (dry-run)", repo.name);
                    continue;
                }

                let git_backup = git_backup.clone();

                join_set.spawn(async move {
                    match git_backup.backup(&repo).await {
                        Ok(id) => Ok((repo, id)),
                        Err(e) => Err((repo, e)),
                    }
                });
            }
        }

        while let Some(fut) = join_set.join_next().await {
            match fut.map_err(|e| errors::system_with_internal(
                "Failed to complete a background backup task due to an internal runtime error.",
                "Please report this issue to us on GitHub with details of what you were doing when it occurred.",
                e))? {
                Ok((repo, id)) => println!(" - {} (backup at {})", repo.name, id),
                Err((repo, e)) => {
                    println!(" - {} (backup failed)", repo.name);
                    eprintln!("{}", e)
                },
            }
        }

        println!();
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(e) = run(args).await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
