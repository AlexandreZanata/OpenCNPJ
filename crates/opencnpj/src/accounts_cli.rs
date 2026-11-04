//! `opencnpj accounts`: manage the accounts database directly (bootstrap the first admin,
//! scripted operations), with or without a running server.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use rayon::prelude::*;
use serde_json::json;

use crate::accounts::{Accounts, Actor, Role, Status, UserFilter};

#[derive(Args)]
pub struct AccountsArgs {
    /// Data directory (the database is `<data-dir>/accounts.sqlite`).
    #[arg(long, default_value = "data")]
    pub data_dir: PathBuf,
    /// Database file (overrides `--data-dir`).
    #[arg(long)]
    pub db: Option<PathBuf>,
    #[command(subcommand)]
    pub cmd: AccountsCmd,
}

#[derive(Subcommand)]
pub enum AccountsCmd {
    /// Create an active account and print its login key once.
    Create {
        #[arg(long)]
        username: String,
        #[arg(long)]
        email: String,
        /// admin | moderator | analyst | user
        #[arg(long, default_value = "user")]
        role: String,
        #[arg(long)]
        plan: Option<String>,
        /// Write the key to this file (mode 0600) instead of printing it.
        #[arg(long)]
        key_file: Option<PathBuf>,
    },
    /// List accounts.
    List {
        #[arg(long)]
        q: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },
    /// Change an account's role.
    SetRole {
        #[arg(long)]
        username: String,
        #[arg(long)]
        role: String,
    },
    /// Move an account to another plan.
    SetPlan {
        #[arg(long)]
        username: String,
        #[arg(long)]
        plan: String,
    },
    /// Revoke the key and sessions and print a new key once.
    RotateKey {
        #[arg(long)]
        username: String,
        #[arg(long)]
        key_file: Option<PathBuf>,
    },
    /// Suspend an account.
    Suspend {
        #[arg(long)]
        username: String,
        #[arg(long)]
        reason: String,
    },
    /// Reactivate a suspended or banned account.
    Reactivate {
        #[arg(long)]
        username: String,
    },
    /// Print the plans.
    Plans,
    /// Create N load-test accounts (`<prefix>00001`…) on a plan and write their keys, one
    /// per line, to a file.
    Seed {
        #[arg(long)]
        count: usize,
        #[arg(long, default_value = "loadtest")]
        prefix: String,
        #[arg(long, default_value = "pro")]
        plan: String,
        #[arg(long)]
        key_file: PathBuf,
    },
}

/// Writes a secret to a new file readable only by the owner.
pub fn write_secret(path: &Path, text: &str) -> Result<()> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o.open(path).with_context(|| format!("creating {}", path.display()))?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

fn emit_key(key: &str, file: Option<&Path>) -> Result<()> {
    match file {
        Some(p) => {
            write_secret(p, &format!("{key}\n"))?;
            eprintln!("key written to {} (mode 0600)", p.display());
        }
        None => println!("{key}"),
    }
    Ok(())
}

pub fn run(a: AccountsArgs) -> Result<()> {
    let db = a.db.clone().unwrap_or_else(|| a.data_dir.join("accounts.sqlite"));
    let acc = Accounts::open(&db, 168)?;
    let actor = Actor::system("cli");
    let find = |u: &str| -> Result<crate::accounts::User> {
        acc.user_by("username", &u.to_ascii_lowercase())?.with_context(|| format!("no user {u}"))
    };
    match a.cmd {
        AccountsCmd::Create { username, email, role, plan, key_file } => {
            let role = Role::parse(&role).context("role must be admin, moderator, analyst or user")?;
            let (u, key) = acc.create_user(&username, &email, role, plan.as_deref(), Status::Active, &actor)?;
            eprintln!("created {} (id {}, role {}, plan {})", u.username, u.id, u.role, u.plan);
            emit_key(&key, key_file.as_deref())
        }
        AccountsCmd::List { q, role, status, limit } => {
            let v = acc.list_users(&UserFilter { q, role, status, limit: Some(limit), ..Default::default() })?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        AccountsCmd::SetRole { username, role } => {
            let u = find(&username)?;
            let u = acc.update_user(u.id, &json!({ "role": role }), &actor)?;
            eprintln!("{} is now {}", u.username, u.role);
            Ok(())
        }
        AccountsCmd::SetPlan { username, plan } => {
            let u = find(&username)?;
            let u = acc.update_user(u.id, &json!({ "plan": plan }), &actor)?;
            eprintln!("{} is now on plan {}", u.username, u.plan);
            Ok(())
        }
        AccountsCmd::RotateKey { username, key_file } => {
            let u = find(&username)?;
            let key = acc.rotate_key(u.id, &actor)?;
            emit_key(&key, key_file.as_deref())
        }
        AccountsCmd::Suspend { username, reason } => {
            let u = find(&username)?;
            acc.set_status(u.id, Status::Suspended, Some(&reason), None, &actor)?;
            eprintln!("{} suspended", u.username);
            Ok(())
        }
        AccountsCmd::Reactivate { username } => {
            let u = find(&username)?;
            acc.set_status(u.id, Status::Active, None, None, &actor)?;
            eprintln!("{} reactivated", u.username);
            Ok(())
        }
        AccountsCmd::Plans => {
            println!("{}", serde_json::to_string_pretty(&acc.plans_json()?)?);
            Ok(())
        }
        AccountsCmd::Seed { count, prefix, plan, key_file } => {
            if acc.plan(&plan).is_none() {
                bail!("unknown plan {plan}");
            }
            // Argon2 hashing dominates: run it in parallel.
            let keys: Vec<String> = (1..=count)
                .into_par_iter()
                .map(|i| {
                    let name = format!("{prefix}{i:05}");
                    acc.create_user(&name, &format!("{name}@loadtest.invalid"), Role::User, Some(&plan), Status::Active, &actor)
                        .map(|x| x.1)
                })
                .collect::<Result<_>>()?;
            write_secret(&key_file, &(keys.join("\n") + "\n"))?;
            eprintln!("created {count} accounts on plan {plan}; keys in {}", key_file.display());
            Ok(())
        }
    }
}
