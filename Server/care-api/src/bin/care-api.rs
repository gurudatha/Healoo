//! API + WebSocket gateway + outbox relay + ws-fanout consumer.
//! With EVENT_BUS=memory it also runs the worker consumers in-process.

use care_api::{
    api::{self, AppState},
    auth::{self, DevVerifier, TokenVerifier},
    config::{AuthMode, BusKind, Config, NetworkMode},
    events::{self, topics, EventHandler, Publisher},
    files, net,
    store::Db,
    worker, ws,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let json_logs = std::env::var("NETWORK_MODE").map(|m| m.eq_ignore_ascii_case("internet")).unwrap_or(false);
    care_api::init_tracing(json_logs);
    let cfg = Config::from_env()?;
    tracing::info!(mode = ?cfg.mode, auth = ?cfg.auth_mode, bus = ?cfg.bus, store = ?cfg.store, instance = %cfg.instance_id, "starting care-api");

    let db = Arc::new(Db::connect(&cfg.cassandra_nodes, &cfg.keyspace).await?);
    let bus = events::build(&cfg).await?;
    let exposure = net::build(&cfg).await?;
    let (file_store, disk) = files::build(&cfg, &exposure.public_base()).await?;

    // Identity: in dev mode keep the concrete verifier too, so /dev/token can mint tokens.
    let (verifier, dev): (Arc<dyn TokenVerifier>, Option<Arc<DevVerifier>>) = match cfg.auth_mode {
        AuthMode::Dev => {
            let d = Arc::new(DevVerifier::new(&cfg.dev_jwt_secret, &cfg.claims_namespace));
            (d.clone() as Arc<dyn TokenVerifier>, Some(d))
        }
        AuthMode::Auth0 => (auth::build(&cfg), None),
    };

    let publisher = Publisher::new(bus.clone(), db.clone());
    let registry = Arc::new(ws::Registry::new());
    let stop = CancellationToken::new();

    // Outbox relay (doc 6.3).
    tokio::spawn(publisher.clone().run_relay(stop.clone()));

    // ws-fanout: a group per instance so every instance sees every event (doc 6.4).
    {
        let fanout: Arc<dyn EventHandler> = Arc::new(ws::Fanout { db: db.clone(), registry: registry.clone(), publisher: publisher.clone() });
        let (bus, stop, group) = (bus.clone(), stop.clone(), format!("ws-fanout-{}", cfg.instance_id));
        tokio::spawn(async move {
            let t = [topics::CHAT, topics::ITEMS, topics::ALERTS, topics::ACCESS_CHANGES];
            if let Err(e) = bus.consume(&group, &t, false, fanout, stop).await { tracing::error!("ws-fanout stopped: {e:#}"); }
        });
    }

    if cfg.bus == BusKind::Memory {
        if cfg.mode == NetworkMode::Internet { tracing::warn!("EVENT_BUS=memory in internet mode: events are lost on restart"); }
        worker::spawn(bus.clone(), db.clone(), file_store.clone(), publisher.clone(), stop.clone());
    }

    let state = AppState::new(cfg, db, verifier, dev, exposure.clone(), file_store, disk, publisher, registry);
    let app = api::router(state);

    let s = stop.clone();
    tokio::spawn(async move { care_api::shutdown_signal().await; tracing::info!("shutting down"); s.cancel(); });
    exposure.serve(app, stop).await
}
