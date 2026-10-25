//! care-admin: sets who the administrators are (Administration_Design.md sections 8 and 10).
//!
//! The usual way is the encrypted administrators file, config/admins/admins.enc, read with the key
//! phrase in ADMINS_KEY. care-api applies it at every start; these commands manage it:
//!
//!   care-admin config init                          new file with the seeded test administrators
//!   care-admin config show                          print the decrypted list
//!   care-admin config add-platform "<name>" <email|-> <Healoo ID|->
//!   care-admin config add-hospital "<name>" <email|-> <Healoo ID|-> <hospital Healoo ID>
//!   care-admin config remove <name, email or Healoo ID>
//!   care-admin config import <file.json>            replace the list with a plain JSON file
//!   care-admin config rekey                         re-encrypt with ADMINS_NEW_KEY
//!   care-admin config apply                         apply now (otherwise at the next care-api start)
//!
//! Without a file, single accounts can be changed directly (the file wins at the next start):
//!
//!   care-admin grant HL-4K7Q2 | revoke HL-4K7Q2 | list
//!
//! Run them in the API container, e.g. `docker compose run --rm care-api care-admin config show`.

use care_api::{admins_file::{self, AdminEntry, AdminsFile, HospitalAdminEntry}, config::Config, model::Role, store::Db};
use anyhow::{anyhow, bail, Result};

fn opt(s: &str) -> Option<String> { (s != "-" && !s.trim().is_empty()).then(|| s.trim().to_string()) }

fn key() -> Result<String> {
    admins_file::location().1.ok_or_else(|| anyhow!("set ADMINS_KEY (the key phrase) first"))
}

async fn db() -> Result<Db> {
    let cfg = Config::from_env()?;
    Db::connect(&cfg.cassandra_nodes, &cfg.keyspace).await
}

fn print(f: &AdminsFile) {
    println!("Platform administrators:");
    for e in &f.platform_admins { println!("  {}  {}  {}", e.name, e.email.as_deref().unwrap_or("-"), e.user_id.as_deref().unwrap_or("-")); }
    println!("Hospital administrators:");
    for e in &f.hospital_admins { println!("  {}  {}  {}  hospital {}", e.name, e.email.as_deref().unwrap_or("-"), e.user_id.as_deref().unwrap_or("-"), e.hospital); }
}

async fn config(args: &[&str]) -> Result<()> {
    let (path, _) = admins_file::location();
    let load = || -> Result<AdminsFile> { admins_file::read(&path, &key()?) };
    let save = |f: &AdminsFile| -> Result<()> {
        admins_file::parse(&serde_json::to_vec(f)?)?;   // same checks as reading
        admins_file::write(&path, f, &key()?)?;
        println!("Saved {} (encrypted). It is applied at the next care-api start, or now with: care-admin config apply", path.display());
        Ok(())
    };
    match args {
        ["init"] => {
            if path.exists() { bail!("{} already exists; use show / add-… / remove", path.display()); }
            save(&admins_file::example())?;
            print(&admins_file::example());
        }
        ["show"] => print(&load()?),
        ["show", "--json"] => println!("{}", serde_json::to_string_pretty(&load()?)?),
        ["add-platform", name, email, id] => {
            let mut f = if path.exists() { load()? } else { AdminsFile::default() };
            let max = care_api::settings::settings().administrators.max_platform_admins;
            if f.platform_admins.len() >= max {
                bail!("the file already lists {max} platform administrators, the most allowed (administrators.max_platform_admins in server.json); remove one first");
            }
            f.platform_admins.push(AdminEntry { name: name.to_string(), email: opt(email), user_id: opt(id) });
            save(&f)?;
        }
        ["add-hospital", name, email, id, hospital] => {
            let mut f = if path.exists() { load()? } else { AdminsFile::default() };
            let max = care_api::settings::settings().administrators.max_admins_per_hospital;
            if f.hospital_admins.iter().filter(|e| e.hospital.eq_ignore_ascii_case(hospital)).count() >= max {
                bail!("the file already lists {max} administrators for {hospital}, the most allowed (administrators.max_admins_per_hospital); remove one first");
            }
            f.hospital_admins.push(HospitalAdminEntry { name: name.to_string(), email: opt(email), user_id: opt(id), hospital: hospital.to_string() });
            save(&f)?;
        }
        ["remove", who] => {
            let mut f = load()?;
            let w = who.to_lowercase();
            let hit = |name: &str, email: &Option<String>, id: &Option<String>| {
                name.to_lowercase() == w || email.as_deref().is_some_and(|e| e.to_lowercase() == w) || id.as_deref().is_some_and(|i| i.to_lowercase() == w)
            };
            let before = f.platform_admins.len() + f.hospital_admins.len();
            f.platform_admins.retain(|e| !hit(&e.name, &e.email, &e.user_id));
            f.hospital_admins.retain(|e| !hit(&e.name, &e.email, &e.user_id));
            if f.platform_admins.len() + f.hospital_admins.len() == before { bail!("nobody called {who} in the file"); }
            save(&f)?;
        }
        ["import", file] => {
            let f = admins_file::parse(&std::fs::read(file)?)?;
            save(&f)?;
            println!("Delete {file} now: it is not encrypted.");
        }
        ["rekey"] => {
            let f = load()?;
            let new = std::env::var("ADMINS_NEW_KEY").ok().filter(|k| !k.is_empty()).ok_or_else(|| anyhow!("set ADMINS_NEW_KEY to the new key phrase"))?;
            admins_file::write(&path, &f, &new)?;
            println!("Re-encrypted {} with the new key phrase. Put it in ADMINS_KEY (deploy/internet/.env) before the next start.", path.display());
        }
        ["apply"] => {
            let r = admins_file::apply(&db().await?, &load()?).await?;
            for l in r.lines { println!("{l}"); }
        }
        _ => bail!("unknown config command; see the top of src/bin/care-admin.rs"),
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    care_api::install_crypto_provider();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    match argv.as_slice() {
        ["config", rest @ ..] => config(rest).await?,
        ["grant", id] | ["revoke", id] => {
            let grant = argv[0] == "grant";
            let db = db().await?;
            let user_id = db.user_id_by_public_id(id).await?.ok_or_else(|| anyhow!("no account with Healoo ID {id}"))?;
            let u = db.user(user_id).await?.ok_or_else(|| anyhow!("no account with Healoo ID {id}"))?;
            let mut roles = u.roles.clone();
            if grant {
                if roles.iter().any(|r| r.is_clinical()) {
                    bail!("{} is a {:?} account; use a separate account for platform administration", u.display_name, u.primary_role());
                }
                let max = care_api::settings::settings().administrators.max_platform_admins;
                if !roles.contains(&Role::AppAdministrator) && db.platform_admin_ids().await?.len() >= max {
                    bail!("there are already {max} platform administrators, the most allowed (administrators.max_platform_admins in server.json); revoke one first");
                }
                roles.insert(Role::AppAdministrator);
            } else {
                roles.remove(&Role::AppAdministrator);
                if roles.is_empty() { roles.insert(Role::Patient); }
            }
            db.set_roles(&u, &roles, u.primary_hospital_id).await?;
            println!("{} ({}) {} a platform administrator.", u.display_name, u.public_id, if grant { "is now" } else { "is no longer" });
            if admins_file::location().0.exists() {
                println!("Note: the administrators file decides at the next care-api start. Use `care-admin config add-platform` / `remove` to make this last.");
            }
        }
        ["list"] => {
            for a in db().await?.all_accounts().await?.into_iter().filter(|a| a.roles.contains(&Role::AppAdministrator)) {
                println!("{}  {}{}", a.public_id, a.display_name, if a.active { "" } else { "  (deactivated)" });
            }
        }
        _ => {
            eprintln!("usage: care-admin config <init|show|add-platform|add-hospital|remove|import|rekey|apply> …");
            eprintln!("       care-admin grant <Healoo ID> | revoke <Healoo ID> | list");
            std::process::exit(2);
        }
    }
    Ok(())
}
