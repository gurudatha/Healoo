//! care-seed: loads the trial accounts, items and chat (doc 10.3). Safe to run twice.

use care_api::{config::Config, files, store::Db};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    care_api::init_tracing(false);
    let cfg = Config::from_env()?;
    let db = Db::connect(&cfg.cassandra_nodes, &cfg.keyspace).await?;
    let base = cfg.public_base_url.clone().unwrap_or_else(|| url::Url::parse("http://localhost/").unwrap());
    let (store, _) = files::build(&cfg, &base).await?;
    if care_api::seed::run(&db, store.as_ref()).await? {
        println!("Seeded. Healoo IDs: {}", care_api::seed::SEED_PUBLIC_IDS.join(", "));
        println!("Lakshmi K. (patient) = HL-2M9P4, Dr. Anitha Rao = HL-7R2C9, City Diagnostics = HL-6L3D2");
    }
    Ok(())
}
