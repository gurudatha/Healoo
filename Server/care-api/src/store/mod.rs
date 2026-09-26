//! Cassandra access (design doc 7). One `Db` wraps the driver session; each submodule adds
//! query methods for one area. Tables are query-shaped (doc 7.1) and denormalised writes that
//! must stay together use logged batches (doc 7.3).

pub mod items;
pub mod misc;
pub mod social;
pub mod users;

use anyhow::{Context, Result};
use dashmap::DashMap;
use scylla::{
    batch::Batch, prepared_statement::PreparedStatement, serialize::batch::BatchValues, serialize::row::SerializeRow,
    QueryResult, Session, SessionBuilder,
};
use std::time::Duration;

pub struct Db {
    session: Session,
    prepared: DashMap<&'static str, PreparedStatement>,
}

impl Db {
    /// Connects, retrying while Cassandra is still starting (it can take a minute in Docker).
    pub async fn connect(nodes: &[String], keyspace: &str) -> Result<Self> {
        let mut attempt = 0;
        loop {
            match SessionBuilder::new().known_nodes(nodes).use_keyspace(keyspace, false).build().await {
                Ok(session) => return Ok(Db { session, prepared: DashMap::new() }),
                Err(e) if attempt < 30 => {
                    attempt += 1;
                    tracing::warn!("Cassandra not ready ({e}); retry {attempt}/30");
                    tokio::time::sleep(Duration::from_secs(4)).await;
                }
                Err(e) => return Err(e).context("connecting to Cassandra"),
            }
        }
    }

    async fn stmt(&self, cql: &'static str) -> Result<PreparedStatement> {
        if let Some(p) = self.prepared.get(cql) {
            return Ok(p.clone());
        }
        let p = self.session.prepare(cql).await.with_context(|| format!("preparing: {cql}"))?;
        self.prepared.insert(cql, p.clone());
        Ok(p)
    }

    pub async fn exec(&self, cql: &'static str, values: impl SerializeRow) -> Result<QueryResult> {
        let p = self.stmt(cql).await?;
        self.session.execute(&p, values).await.with_context(|| format!("executing: {cql}"))
    }

    pub async fn rows<T: scylla::cql_to_rust::FromRow>(&self, cql: &'static str, values: impl SerializeRow) -> Result<Vec<T>> {
        let res = self.exec(cql, values).await?;
        Ok(res.rows_typed::<T>()?.collect::<Result<Vec<T>, _>>()?)
    }

    pub async fn one<T: scylla::cql_to_rust::FromRow>(&self, cql: &'static str, values: impl SerializeRow) -> Result<Option<T>> {
        Ok(self.rows::<T>(cql, values).await?.into_iter().next())
    }

    /// Logged batch of prepared statements; `values` is a tuple with one entry per statement.
    pub async fn batch(&self, cqls: &[&'static str], values: impl BatchValues) -> Result<()> {
        let mut b = Batch::default();
        for c in cqls {
            b.append_statement(self.stmt(c).await?);
        }
        self.session.batch(&b, values).await.context("executing batch")?;
        Ok(())
    }

    pub async fn ping(&self) -> bool {
        self.session.query("SELECT release_version FROM system.local", ()).await.is_ok()
    }
}
