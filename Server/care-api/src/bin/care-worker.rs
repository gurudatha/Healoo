//! care-worker: audit-writer + media processor (doc 6.4, 7.4). Needs EVENT_BUS=kafka.

use care_api::{config::{BusKind, Config}, events::{self, Publisher}, files, store::Db, worker};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    care_api::init_tracing(std::env::var("NETWORK_MODE").map(|m| m.eq_ignore_ascii_case("internet")).unwrap_or(false));
    let cfg = Config::from_env()?;
    if cfg.bus == BusKind::Memory {
        anyhow::bail!("care-worker needs EVENT_BUS=kafka; with the memory bus the worker runs inside care-api");
    }
    let db = Arc::new(Db::connect(&cfg.cassandra_nodes, &cfg.keyspace).await?);
    let bus = events::build(&cfg).await?;
    // Public base only matters for URL signing, which the worker does not do.
    let base = cfg.public_base_url.clone().unwrap_or_else(|| url::Url::parse("http://localhost/").unwrap());
    let (store, _) = files::build(&cfg, &base).await?;
    let publisher = Publisher::new(bus.clone(), db.clone());
    let stop = CancellationToken::new();
    tokio::spawn(publisher.clone().run_relay(stop.clone()));
    let tasks = worker::spawn(bus, db, store, publisher, stop.clone());
    tracing::info!("care-worker running: audit-writer, media, alert scheduler");
    care_api::shutdown_signal().await;
    stop.cancel();
    for t in tasks { let _ = t.await; }
    Ok(())
}
