use std::io;
use std::io::IsTerminal;
use std::io::Read;
use std::io::Write;
use std::thread;
use std::time::Duration;

use clap::Parser;
use clap::Subcommand;
use serde_json::Value;

use crate::client::Client;
use crate::config;
use crate::config::Config;
use crate::error::Error;

#[derive(Parser)]
#[command(
    name = "amendable",
    about = "Create and use Amendable Git repositories from the command line.",
    version,
    arg_required_else_help = true
)]
struct Cli {
    /// Internal: talk to the Amendable staging server.
    #[arg(long, global = true, hide = true)]
    staging: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a browser grant and store an access token.
    Login {
        /// Label shown on the grant page. Defaults to this machine.
        #[arg(long)]
        hostname: Option<String>,
        /// How often to poll for the grant.
        #[arg(long, default_value_t = 2.0)]
        poll_seconds: f64,
    },
    /// Remove the stored access token.
    Logout,
    /// Show the stored username and current usage.
    Whoami {
        #[arg(long)]
        json: bool,
    },
    /// Show repository, storage, and transfer usage.
    Usage {
        #[arg(long)]
        json: bool,
    },
    /// Git credential helper. Configure with:
    /// git config --global credential.https://amendable.io.helper '!amendable git-credential'
    #[command(name = "git-credential")]
    GitCredential {
        /// Action appended by Git. `get` prints username and password.
        action: Option<String>,
    },
    /// Exchange an OIDC ID token for a short-lived Amendable access token.
    #[command(name = "oidc-exchange")]
    OidcExchange {
        /// OIDC ID token from GitHub Actions, GitLab, or another issuer.
        #[arg(long)]
        id_token: String,
        #[arg(long)]
        json: bool,
    },
    /// Create, list, clone, and delete repositories.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Create and list access tokens.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    /// Create, ping, and inspect webhooks.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Webhook {
        #[command(subcommand)]
        command: WebhookCommand,
    },
}

#[derive(Subcommand)]
enum RepoCommand {
    /// List repositories the current token can see.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create a repository. Needs the API_REPOS_WRITE grant (or ALL).
    Create {
        /// Repository name (lowercase, hyphens, underscores).
        name: String,
        #[arg(short, long)]
        description: Option<String>,
        /// Verified BYO bucket id. Optional.
        #[arg(long)]
        storage_bucket_id: Option<String>,
        /// Clone into ./NAME after create.
        #[arg(long)]
        clone: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show one repository.
    Get {
        /// NAME or owner/NAME.
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Delete a repository. Needs API_REPOS_WRITE (or ALL).
    Delete {
        /// NAME or owner/NAME.
        name: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Clone a repository over HTTPS using your stored token.
    Clone {
        /// NAME or owner/NAME.
        name: String,
        /// Target directory.
        directory: Option<String>,
    },
    /// Print the HTTPS clone URL.
    Url {
        /// NAME or owner/NAME.
        name: String,
    },
}

#[derive(Subcommand)]
enum TokenCommand {
    /// List access tokens. Needs ALL + ALL_REPO on the calling token.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create an access token. The secret is printed once.
    Create {
        #[arg(long)]
        name: Option<String>,
        /// ALL_REPO or SELECTED_REPO.
        #[arg(long, default_value = "ALL_REPO")]
        scope: String,
        /// Comma-separated grants. Example: GIT_HTTP_READ,GIT_HTTP_WRITE,API_REPOS_WRITE.
        #[arg(long, default_value = "ALL")]
        grants: String,
        /// Comma-separated repo UUIDs for SELECTED_REPO.
        #[arg(long)]
        repository_ids: Option<String>,
        /// Comma-separated Amendable branch names this token may push. Empty means all branches.
        #[arg(long)]
        branches: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Delete an access token.
    Delete { token_id: String },
}

#[derive(Subcommand)]
enum WebhookCommand {
    /// List account webhooks.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create a webhook. The signing secret is printed once.
    Create {
        url: String,
        /// Comma-separated: push,create,delete,ping.
        #[arg(long, default_value = "push")]
        events: String,
        #[arg(long)]
        json: bool,
    },
    /// Send a ping event so you can verify HMAC and connectivity.
    Ping {
        webhook_id: String,
        #[arg(long)]
        json: bool,
    },
    /// List recent deliveries.
    Deliveries {
        webhook_id: String,
        #[arg(long)]
        include_payload: bool,
        #[arg(long)]
        json: bool,
    },
    /// Delete a webhook.
    Delete { webhook_id: String },
}

pub fn try_run() -> Result<(), Error> {
    let cli = Cli::parse();
    execute(cli)
}

fn execute(cli: Cli) -> Result<(), Error> {
    if cli.staging {
        config::apply_staging_env();
    }
    match cli.command {
        Commands::Login {
            hostname,
            poll_seconds,
        } => login(hostname, poll_seconds, cli.staging),
        Commands::Logout => logout(),
        Commands::Whoami { json } | Commands::Usage { json } => whoami(json),
        Commands::GitCredential { action } => git_credential(action.as_deref()),
        Commands::OidcExchange { id_token, json } => oidc_exchange(&id_token, json),
        Commands::Repo { command } => match command {
            RepoCommand::List { json } => repo_list(json),
            RepoCommand::Create {
                name,
                description,
                storage_bucket_id,
                clone,
                json,
            } => repo_create(
                &name,
                description.as_deref(),
                storage_bucket_id.as_deref(),
                clone,
                json,
            ),
            RepoCommand::Get { name, json } => repo_get(&name, json),
            RepoCommand::Delete { name, yes } => repo_delete(&name, yes),
            RepoCommand::Clone { name, directory } => repo_clone(&name, directory.as_deref()),
            RepoCommand::Url { name } => repo_url(&name),
        },
        Commands::Token { command } => match command {
            TokenCommand::List { json } => token_list(json),
            TokenCommand::Create {
                name,
                scope,
                grants,
                repository_ids,
                branches,
                json,
            } => token_create(
                name.as_deref(),
                &scope,
                &grants,
                repository_ids.as_deref(),
                branches.as_deref(),
                json,
            ),
            TokenCommand::Delete { token_id } => token_delete(&token_id),
        },
        Commands::Webhook { command } => match command {
            WebhookCommand::List { json } => webhook_list(json),
            WebhookCommand::Create { url, events, json } => webhook_create(&url, &events, json),
            WebhookCommand::Ping { webhook_id, json } => webhook_ping(&webhook_id, json),
            WebhookCommand::Deliveries {
                webhook_id,
                include_payload,
                json,
            } => webhook_deliveries(&webhook_id, include_payload, json),
            WebhookCommand::Delete { webhook_id } => webhook_delete(&webhook_id),
        },
    }
}

fn login(hostname: Option<String>, poll_seconds: f64, staging: bool) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    if staging {
        cfg.use_staging();
    }
    let host = match hostname {
        Some(value) => value,
        None => current_hostname(),
    };
    let client = Client::new(&cfg.api_url, cfg.token.as_deref())?;
    let session = client.create_auth_session(&host)?;
    let auth_url = required_str(&session, "auth_url")?;
    let code = required_str(&session, "code")?;
    let session_id = required_str(&session, "id")?;
    let secret_token = required_str(&session, "secret_token")?;
    println!("Compare this code with the page in your browser:");
    println!("  {code}");
    println!("Grant URL: {auth_url}");
    let _ = open::that(&auth_url);
    println!("Waiting for you to grant access...");
    let delay = Duration::from_secs_f64(poll_seconds.max(0.0));
    loop {
        thread::sleep(delay);
        let result = client.poll_auth_session(&session_id, &secret_token)?;
        if let Some(token) = result.get("token").and_then(Value::as_str) {
            cfg.token = Some(token.to_string());
            if let Some(repos) = result.get("repositories").and_then(Value::as_array) {
                if let Some(first) = repos.first().and_then(Value::as_str) {
                    if let Some((username, _)) = first.split_once('/') {
                        cfg.username = Some(username.to_string());
                    }
                }
            }
            cfg.save()?;
            println!(
                "Logged in. Token saved to {}",
                config::config_path().display()
            );
            if let Some(username) = &cfg.username {
                println!("Username: {username}");
            }
            return Ok(());
        }
    }
}

fn logout() -> Result<(), Error> {
    let mut cfg = Config::load()?;
    cfg.token = None;
    cfg.save()?;
    println!(
        "Logged out. {} no longer has a token.",
        config::config_path().display()
    );
    Ok(())
}

fn whoami(json: bool) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let usage = client.get_usage()?;
    let username = infer_username(&mut cfg, &client)?;
    let payload = serde_json::json!({
        "username": username,
        "usage": usage,
    });
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    println!("API: {}", cfg.api_url);
    println!(
        "Username: {}",
        username.unwrap_or_else(|| "(unknown until you create a repository)".to_string())
    );
    print_quota("Repos", &usage["repos"], "");
    print_quota("Storage", &usage["storage_bytes"], " bytes");
    print_quota("Transfer this month", &usage["transfer_bytes"], " bytes");
    Ok(())
}

fn git_credential(action: Option<&str>) -> Result<(), Error> {
    let cfg = Config::load()?;
    if cfg.token.is_none() {
        return Err(Error::NotLoggedIn);
    }
    let action = action.unwrap_or("").trim().to_ascii_lowercase();
    if action != "get" {
        // Git also sends "store" and "erase". Ignore them.
        return Ok(());
    }
    let mut _fields = String::new();
    io::stdin().read_to_string(&mut _fields)?;
    let username = cfg.username.as_deref().unwrap_or("amendable");
    let token = cfg.require_token()?;
    let mut stdout = io::stdout();
    write!(stdout, "username={username}\npassword={token}\n\n")?;
    Ok(())
}

fn oidc_exchange(id_token: &str, json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, None)?;
    let payload = client.exchange_oidc_token(id_token)?;
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    println!("{}", required_str(&payload, "token")?);
    if let Some(expires) = payload.get("expires_at").and_then(Value::as_str) {
        println!("expires_at: {expires}");
    }
    Ok(())
}

fn repo_list(json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let payload = client.list_repositories()?;
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    let repos = payload
        .get("repositories")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if repos.is_empty() {
        println!("No repositories yet. Create one with `amendable repo create`.");
        return Ok(());
    }
    let mut rows = Vec::new();
    for repo in &repos {
        let username = repo.get("username").and_then(Value::as_str).unwrap_or("");
        let name = repo.get("name").and_then(Value::as_str).unwrap_or("");
        let active = if repo.get("active").and_then(Value::as_bool).unwrap_or(false) {
            "yes"
        } else {
            "no"
        };
        let source = repo
            .get("storage_source")
            .and_then(Value::as_str)
            .unwrap_or("platform");
        rows.push(vec![
            format!("{username}/{name}"),
            active.to_string(),
            source.to_string(),
        ]);
    }
    print_table(&["NAME", "ACTIVE", "STORAGE"], &rows);
    Ok(())
}

fn repo_create(
    name: &str,
    description: Option<&str>,
    storage_bucket_id: Option<&str>,
    clone: bool,
    json: bool,
) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let repo = client.create_repository(name, description, storage_bucket_id)?;
    if let Some(username) = repo.get("username").and_then(Value::as_str) {
        cfg.username = Some(username.to_string());
        cfg.save()?;
    }
    if json {
        print_json(&repo)?;
    } else {
        let username = required_str(&repo, "username")?;
        let repo_name = required_str(&repo, "name")?;
        let url = config::git_clone_url(&cfg.app_url, &username, &repo_name);
        println!("Created {username}/{repo_name}");
        println!("Clone: {url}");
    }
    if clone {
        let username = required_str(&repo, "username")?;
        let repo_name = required_str(&repo, "name")?;
        clone_repo(&cfg, &username, &repo_name, None)?;
    }
    Ok(())
}

fn repo_get(name: &str, json: bool) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let username = infer_username(&mut cfg, &client)?;
    let (owner, repo_name) = parse_repo(name, username.as_deref())?;
    let repo = client.get_repository(&owner, &repo_name)?;
    if json {
        print_json(&repo)?;
        return Ok(());
    }
    let username = required_str(&repo, "username")?;
    let repo_name = required_str(&repo, "name")?;
    let url = config::git_clone_url(&cfg.app_url, &username, &repo_name);
    println!("{username}/{repo_name}");
    println!(
        "Active: {}",
        repo.get("active")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".into())
    );
    println!(
        "Storage: {}",
        repo.get("storage_source")
            .and_then(Value::as_str)
            .unwrap_or("null")
    );
    println!("Clone: {url}");
    Ok(())
}

fn repo_delete(name: &str, yes: bool) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let username = infer_username(&mut cfg, &client)?;
    let (owner, repo_name) = parse_repo(name, username.as_deref())?;
    if !yes {
        confirm_delete(&owner, &repo_name)?;
    }
    client.delete_repository(&owner, &repo_name)?;
    println!("Deleted {owner}/{repo_name}");
    Ok(())
}

fn repo_clone(name: &str, directory: Option<&str>) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let username = infer_username(&mut cfg, &client)?;
    let (owner, repo_name) = parse_repo(name, username.as_deref())?;
    clone_repo(&cfg, &owner, &repo_name, directory)
}

fn repo_url(name: &str) -> Result<(), Error> {
    let mut cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let username = infer_username(&mut cfg, &client)?;
    let (owner, repo_name) = parse_repo(name, username.as_deref())?;
    println!(
        "{}",
        config::git_clone_url(&cfg.app_url, &owner, &repo_name)
    );
    Ok(())
}

fn token_list(json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let payload = client.list_access_tokens()?;
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    let mut rows = Vec::new();
    if let Some(items) = payload.get("access_tokens").and_then(Value::as_array) {
        for item in items {
            let grants = match item.get("grants") {
                Some(Value::Array(values)) => values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            rows.push(vec![
                item.get("id").and_then(Value::as_str).unwrap_or("").into(),
                item.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                item.get("scope")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                grants,
            ]);
        }
    }
    print_table(&["ID", "NAME", "SCOPE", "GRANTS"], &rows);
    Ok(())
}

fn token_create(
    name: Option<&str>,
    scope: &str,
    grants: &str,
    repository_ids: Option<&str>,
    branches: Option<&str>,
    json: bool,
) -> Result<(), Error> {
    let cfg = Config::load()?;
    let grant_list = split_csv(grants);
    let repo_ids = repository_ids.map(split_csv);
    let allowed_branches = branches.map(split_csv);
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let created = client.create_access_token(
        name,
        scope,
        &grant_list,
        repo_ids.as_deref(),
        allowed_branches.as_deref(),
    )?;
    if json {
        print_json(&created)?;
        return Ok(());
    }
    println!("Created token {}", required_str(&created, "id")?);
    if let Some(token) = created.get("token").and_then(Value::as_str) {
        println!("Secret (shown once):");
        println!("{token}");
    }
    Ok(())
}

fn token_delete(token_id: &str) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    client.delete_access_token(token_id)?;
    println!("Deleted {token_id}");
    Ok(())
}

fn webhook_list(json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let payload = client.list_webhooks()?;
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    let mut rows = Vec::new();
    if let Some(hooks) = payload.get("webhooks").and_then(Value::as_array) {
        for hook in hooks {
            let events = match hook.get("events") {
                Some(Value::Array(values)) => values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            let active = if hook.get("active").and_then(Value::as_bool).unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            rows.push(vec![
                hook.get("id").and_then(Value::as_str).unwrap_or("").into(),
                hook.get("url").and_then(Value::as_str).unwrap_or("").into(),
                events,
                active.into(),
            ]);
        }
    }
    print_table(&["ID", "URL", "EVENTS", "ACTIVE"], &rows);
    Ok(())
}

fn webhook_create(url: &str, events: &str, json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let event_list = split_csv(events);
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let created = client.create_webhook(url, &event_list)?;
    if json {
        print_json(&created)?;
        return Ok(());
    }
    println!("Created webhook {}", required_str(&created, "id")?);
    if let Some(secret) = created.get("secret").and_then(Value::as_str) {
        println!("Signing secret (shown once):");
        println!("{secret}");
    }
    Ok(())
}

fn webhook_ping(webhook_id: &str, json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let payload = client.ping_webhook(webhook_id)?;
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    println!(
        "Ping queued. delivery_id={}",
        required_str(&payload, "delivery_id")?
    );
    Ok(())
}

fn webhook_deliveries(webhook_id: &str, include_payload: bool, json: bool) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    let payload = client.list_webhook_deliveries(webhook_id, include_payload)?;
    if json {
        print_json(&payload)?;
        return Ok(());
    }
    let mut rows = Vec::new();
    if let Some(deliveries) = payload.get("deliveries").and_then(Value::as_array) {
        for row in deliveries {
            rows.push(vec![
                row.get("id").and_then(Value::as_str).unwrap_or("").into(),
                row.get("event_type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                row.get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                row.get("response_code")
                    .map(|v| match v {
                        Value::Null => String::new(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default(),
            ]);
        }
    }
    print_table(&["ID", "TYPE", "STATUS", "HTTP"], &rows);
    Ok(())
}

fn webhook_delete(webhook_id: &str) -> Result<(), Error> {
    let cfg = Config::load()?;
    let client = Client::new(&cfg.api_url, Some(cfg.require_token()?))?;
    client.delete_webhook(webhook_id)?;
    println!("Deleted {webhook_id}");
    Ok(())
}

fn clone_repo(
    cfg: &Config,
    username: &str,
    name: &str,
    directory: Option<&str>,
) -> Result<(), Error> {
    let url = config::git_clone_url(&cfg.app_url, username, name);
    let token = cfg.require_token()?;
    let mut parsed = reqwest::Url::parse(&url).map_err(|err| Error::message(err.to_string()))?;
    let _ = parsed.set_username(username);
    let _ = parsed.set_password(Some(token));
    let mut args = vec!["clone".to_string(), parsed.to_string()];
    if let Some(directory) = directory {
        args.push(directory.to_string());
    }
    let status = std::process::Command::new("git").args(&args).status()?;
    if !status.success() {
        return Err(Error::Git(status.code().unwrap_or(1)));
    }
    let target = directory.unwrap_or(name);
    let _ = std::process::Command::new("git")
        .args(["-C", target, "remote", "set-url", "origin", &url])
        .status();
    let _ = std::process::Command::new("git")
        .args([
            "-C",
            target,
            "config",
            "credential.helper",
            "!amendable git-credential",
        ])
        .status();
    Ok(())
}

fn infer_username(cfg: &mut Config, client: &Client) -> Result<Option<String>, Error> {
    if let Some(username) = &cfg.username {
        return Ok(Some(username.clone()));
    }
    let payload = client.list_repositories()?;
    let repos = payload
        .get("repositories")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(username) = repos
        .first()
        .and_then(|repo| repo.get("username"))
        .and_then(Value::as_str)
    {
        cfg.username = Some(username.to_string());
        cfg.save()?;
        return Ok(cfg.username.clone());
    }
    Ok(None)
}

fn parse_repo(value: &str, username: Option<&str>) -> Result<(String, String), Error> {
    if let Some((owner, name)) = value.split_once('/') {
        return Ok((owner.to_string(), name.to_string()));
    }
    match username {
        Some(username) => Ok((username.to_string(), value.to_string())),
        None => Err(Error::message(
            "Pass owner/name, or set a username with `amendable login`.",
        )),
    }
}

fn confirm_delete(owner: &str, repo_name: &str) -> Result<(), Error> {
    if !io::stdin().is_terminal() {
        return Err(Error::message(format!(
            "Delete {owner}/{repo_name}? Pass --yes to confirm."
        )));
    }
    eprint!("Delete {owner}/{repo_name}? [y/N] ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    match line.trim() {
        "y" | "Y" | "yes" | "YES" => Ok(()),
        _ => Err(Error::Aborted),
    }
}

fn print_json(payload: &Value) -> Result<(), Error> {
    println!("{}", serde_json::to_string_pretty(payload)?);
    Ok(())
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    let format_row = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                format!(
                    "{cell:<width$}",
                    width = widths.get(i).copied().unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let header_cells: Vec<String> = headers.iter().map(|s| (*s).to_string()).collect();
    println!("{}", format_row(&header_cells));
    for row in rows {
        println!("{}", format_row(row));
    }
}

fn print_quota(label: &str, value: &Value, unit: &str) {
    let used = value
        .get("used")
        .map(value_to_display)
        .unwrap_or_else(|| "0".into());
    match value.get("quota") {
        Some(Value::Null) | None => println!("{label}: {used}{unit}"),
        Some(quota) => println!("{label}: {used}{unit} / {}", value_to_display(quota)),
    }
}

fn value_to_display(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

fn required_str(payload: &Value, key: &str) -> Result<String, Error> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::message(format!("missing field {key} in API response")))
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn current_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
