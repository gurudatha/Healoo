//! DataItem v2 header: items, items_by_owner, items_by_grantee, grants_by_id, grants_by_owner.
//! Children (messages, attachments, appointments, alerts) are in store/parts.rs.

use super::Db;
use crate::model::*;
use anyhow::{anyhow, Result};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Item {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub primary_kind: PrimaryKind,
    pub kinds: HashSet<String>,
    pub title: String,
    pub keywords: HashSet<String>,
    pub grants: Vec<GrantUdt>,
    pub status: ItemStatus,
    pub closure: Option<ClosureUdt>,
    pub has_report_files: bool,
    pub links: Vec<String>,
    pub created_by: Uuid,
    pub created_by_name: String,
    pub created_at: CqlTimestamp,
    pub updated_at: CqlTimestamp,
}

impl Item {
    pub fn grant_list(&self) -> Vec<Grant> { self.grants.iter().filter_map(Grant::from_udt).collect() }

    pub fn facts(&self) -> crate::policy::ItemFacts {
        crate::policy::ItemFacts {
            owner_id: self.owner_id,
            created_by: self.created_by,
            has_report_files: self.has_report_files,
            grants: self.grant_list().iter().map(|g| (g.grantee_type, g.grantee_id)).collect(),
        }
    }

    /// Owner plus every user granted directly (hospital grants are expanded by the fan-out).
    pub fn participants(&self) -> Vec<Uuid> {
        let mut v = vec![self.owner_id];
        for g in self.grant_list() {
            if g.grantee_type == GranteeType::User && !v.contains(&g.grantee_id) { v.push(g.grantee_id); }
        }
        v
    }

    /// Event audience: owner, direct grantees and "hospital:<id>" for hospital grants.
    pub fn audience(&self) -> Vec<String> {
        let mut a = vec![self.owner_id.to_string()];
        a.extend(self.grant_list().iter().map(|g| match g.grantee_type {
            GranteeType::User => g.grantee_id.to_string(),
            GranteeType::Hospital => format!("hospital:{}", g.grantee_id),
        }));
        a
    }
}

#[derive(scylla::macros::FromRow)]
struct ItemRow {
    item_id: CqlTimeuuid,
    owner_id: Uuid,
    primary_kind: Option<String>,
    kinds: Option<HashSet<String>>,
    title: Option<String>,
    keywords: Option<HashSet<String>>,
    access_list: Option<Vec<GrantUdt>>,
    status: Option<String>,
    closure: Option<ClosureUdt>,
    has_report_files: Option<bool>,
    links: Option<Vec<String>>,
    created_by: Option<Uuid>,
    created_by_name: Option<String>,
    created_at: Option<CqlTimestamp>,
    updated_at: Option<CqlTimestamp>,
}

impl From<ItemRow> for Item {
    fn from(r: ItemRow) -> Self {
        let primary_kind = r.primary_kind.as_deref().and_then(parse).unwrap_or(PrimaryKind::Report);
        Item {
            id: un(r.item_id),
            owner_id: r.owner_id,
            primary_kind,
            kinds: r.kinds.unwrap_or_default(),
            title: r.title.unwrap_or_else(|| primary_kind.label().to_string()),
            keywords: r.keywords.unwrap_or_default(),
            grants: r.access_list.unwrap_or_default(),
            status: r.status.as_deref().and_then(parse).unwrap_or(ItemStatus::Open),
            closure: r.closure,
            has_report_files: r.has_report_files.unwrap_or(false),
            links: r.links.unwrap_or_default(),
            created_by: r.created_by.unwrap_or(r.owner_id),
            created_by_name: r.created_by_name.unwrap_or_default(),
            created_at: r.created_at.unwrap_or(CqlTimestamp(0)),
            updated_at: r.updated_at.unwrap_or(CqlTimestamp(0)),
        }
    }
}

/// Row of a list partition: enough for list screens without loading the item.
#[derive(Clone, Debug)]
pub struct ItemSummary {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub primary_kind: PrimaryKind,
    pub kinds: HashSet<String>,
    pub title: String,
    pub status: ItemStatus,
    pub updated_at: CqlTimestamp,
}

const ITEM_Q: &str = "SELECT item_id, owner_id, primary_kind, kinds, title, keywords, access_list, status, closure, has_report_files, links, created_by, created_by_name, created_at, updated_at FROM items WHERE item_id = ?";

impl Db {
    pub async fn item(&self, id: Uuid) -> Result<Option<Item>> {
        Ok(self.one::<ItemRow>(ITEM_Q, (tu(id),)).await?.map(Item::from))
    }

    /// Writes the header, its owner row, and one grantee row + grant index rows per grant.
    pub async fn insert_item(&self, it: &Item) -> Result<()> {
        let (st, pk) = (text(&it.status), text(&it.primary_kind));
        self.exec(
            "INSERT INTO items (item_id, owner_id, primary_kind, kinds, title, keywords, access_list, status, closure, has_report_files, links, created_by, created_by_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (tu(it.id), it.owner_id, &pk, &it.kinds, &it.title, &it.keywords, &it.grants, &st, &it.closure,
             it.has_report_files, &it.links, it.created_by, &it.created_by_name, it.created_at, it.updated_at),
        ).await?;
        self.exec(
            "INSERT INTO items_by_owner (owner_id, status, item_id, primary_kind, kinds, title, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (it.owner_id, &st, tu(it.id), &pk, &it.kinds, &it.title, it.updated_at),
        ).await?;
        for g in &it.grants { self.index_grant(it, g).await?; }
        Ok(())
    }

    async fn index_grant(&self, it: &Item, g: &GrantUdt) -> Result<()> {
        let grant = Grant::from_udt(g).ok_or_else(|| anyhow!("incomplete grant"))?;
        let (st, pk) = (text(&it.status), text(&it.primary_kind));
        self.batch(
            &[
                "INSERT INTO items_by_grantee (grantee_key, status, item_id, owner_id, primary_kind, kinds, title, updated_at, via_hospital_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                "INSERT INTO grants_by_id (grant_id, item_id, owner_id, grantee_key) VALUES (?, ?, ?, ?)",
                "INSERT INTO grants_by_owner (owner_id, grant_id, item_id, item_title, primary_kind, grantee_type, grantee_id, grantee_name) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            ],
            (
                (grant.key(), &st, tu(it.id), it.owner_id, &pk, &it.kinds, &it.title, it.updated_at, grant.via_hospital_id),
                (grant.grant_id, tu(it.id), it.owner_id, grant.key()),
                (it.owner_id, grant.grant_id, tu(it.id), &it.title, &pk, text(&grant.grantee_type), grant.grantee_id, &grant.grantee_name),
            ),
        ).await
    }

    pub async fn add_grant(&self, it: &Item, g: GrantUdt) -> Result<()> {
        self.exec("UPDATE items SET access_list = access_list + ?, updated_at = ? WHERE item_id = ?", (vec![g.clone()], now_ts(), tu(it.id))).await?;
        self.index_grant(it, &g).await
    }

    pub async fn remove_grant(&self, it: &Item, g: &GrantUdt) -> Result<()> {
        let grant = Grant::from_udt(g).ok_or_else(|| anyhow!("incomplete grant"))?;
        self.batch(
            &[
                "UPDATE items SET access_list = access_list - ?, updated_at = ? WHERE item_id = ?",
                "DELETE FROM items_by_grantee WHERE grantee_key = ? AND status = ? AND item_id = ?",
                "DELETE FROM grants_by_id WHERE grant_id = ?",
                "DELETE FROM grants_by_owner WHERE owner_id = ? AND grant_id = ?",
            ],
            (
                (vec![g.clone()], now_ts(), tu(it.id)),
                (grant.key(), text(&it.status), tu(it.id)),
                (grant.grant_id,),
                (it.owner_id, grant.grant_id),
            ),
        ).await
    }

    pub async fn grant_lookup(&self, grant_id: Uuid) -> Result<Option<(Uuid, Uuid)>> {
        Ok(self.one::<(CqlTimeuuid, Uuid)>("SELECT item_id, owner_id FROM grants_by_id WHERE grant_id = ?", (grant_id,))
            .await?.map(|(i, o)| (un(i), o)))
    }

    pub async fn grants_by_owner(&self, owner: Uuid) -> Result<Vec<OwnedGrantDto>> {
        let rows = self.rows::<(Uuid, CqlTimeuuid, Option<String>, Option<String>, Option<String>, Uuid, Option<String>)>(
            "SELECT grant_id, item_id, item_title, primary_kind, grantee_type, grantee_id, grantee_name FROM grants_by_owner WHERE owner_id = ?",
            (owner,)).await?;
        Ok(rows.into_iter().map(|r| OwnedGrantDto {
            grant_id: r.0,
            item_id: un(r.1),
            item_title: r.2.unwrap_or_default(),
            primary_kind: r.3.as_deref().and_then(parse).unwrap_or(PrimaryKind::Report),
            grantee_type: r.4.as_deref().and_then(parse).unwrap_or(GranteeType::User),
            grantee_id: r.5,
            grantee_name: r.6.unwrap_or_default(),
        }).collect())
    }

    pub async fn owner_partition(&self, owner: Uuid, status: ItemStatus, limit: i32) -> Result<Vec<ItemSummary>> {
        let rows = self.rows::<(CqlTimeuuid, Option<String>, Option<HashSet<String>>, Option<String>, Option<CqlTimestamp>)>(
            "SELECT item_id, primary_kind, kinds, title, updated_at FROM items_by_owner WHERE owner_id = ? AND status = ? LIMIT ?",
            (owner, text(&status), limit)).await?;
        Ok(rows.into_iter().map(|r| summary(un(r.0), owner, r.1, r.2, r.3, r.4, status)).collect())
    }

    pub async fn grantee_partition(&self, key: &str, status: ItemStatus, limit: i32) -> Result<Vec<ItemSummary>> {
        let rows = self.rows::<(CqlTimeuuid, Uuid, Option<String>, Option<HashSet<String>>, Option<String>, Option<CqlTimestamp>)>(
            "SELECT item_id, owner_id, primary_kind, kinds, title, updated_at FROM items_by_grantee WHERE grantee_key = ? AND status = ? LIMIT ?",
            (key, text(&status), limit)).await?;
        Ok(rows.into_iter().map(|r| summary(un(r.0), r.1, r.2, r.3, r.4, r.5, status)).collect())
    }

    /// Records that the item now contains a part of `kind` (D2) and bumps updated_at, in the
    /// header and in every list row so list icons stay right.
    pub async fn add_kind(&self, it: &Item, kind: &str) -> Result<()> {
        let now = now_ts();
        let mut kinds = it.kinds.clone();
        kinds.insert(kind.to_string());
        let st = text(&it.status);
        self.exec("UPDATE items SET kinds = ?, updated_at = ? WHERE item_id = ?", (&kinds, now, tu(it.id))).await?;
        self.exec("UPDATE items_by_owner SET kinds = ?, updated_at = ? WHERE owner_id = ? AND status = ? AND item_id = ?",
            (&kinds, now, it.owner_id, &st, tu(it.id))).await?;
        for g in it.grant_list() {
            self.exec("UPDATE items_by_grantee SET kinds = ?, updated_at = ? WHERE grantee_key = ? AND status = ? AND item_id = ?",
                (&kinds, now, g.key(), &st, tu(it.id))).await?;
        }
        Ok(())
    }

    pub async fn set_has_report_files(&self, id: Uuid) -> Result<()> {
        self.exec("UPDATE items SET has_report_files = true WHERE item_id = ?", (tu(id),)).await?;
        Ok(())
    }

    pub async fn set_title_keywords(&self, it: &Item, title: Option<&str>, keywords: Option<&HashSet<String>>) -> Result<()> {
        let now = now_ts();
        if let Some(k) = keywords {
            self.exec("UPDATE items SET keywords = ?, updated_at = ? WHERE item_id = ?", (k, now, tu(it.id))).await?;
        }
        if let Some(t) = title {
            let st = text(&it.status);
            self.exec("UPDATE items SET title = ?, updated_at = ? WHERE item_id = ?", (t, now, tu(it.id))).await?;
            self.exec("UPDATE items_by_owner SET title = ? WHERE owner_id = ? AND status = ? AND item_id = ?", (t, it.owner_id, &st, tu(it.id))).await?;
            for g in it.grant_list() {
                self.exec("UPDATE items_by_grantee SET title = ? WHERE grantee_key = ? AND status = ? AND item_id = ?", (t, g.key(), &st, tu(it.id))).await?;
            }
        }
        Ok(())
    }

    /// OPEN <-> CLOSED: moves list rows between status partitions and sets or clears the closure.
    pub async fn set_status(&self, it: &Item, to: ItemStatus, closure: Option<ClosureUdt>) -> Result<()> {
        let now = now_ts();
        let (from_s, to_s, pk) = (text(&it.status), text(&to), text(&it.primary_kind));
        self.exec("UPDATE items SET status = ?, closure = ?, updated_at = ? WHERE item_id = ?", (&to_s, &closure, now, tu(it.id))).await?;
        if it.status == to { return Ok(()); }
        self.batch(
            &[
                "DELETE FROM items_by_owner WHERE owner_id = ? AND status = ? AND item_id = ?",
                "INSERT INTO items_by_owner (owner_id, status, item_id, primary_kind, kinds, title, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            ],
            (
                (it.owner_id, &from_s, tu(it.id)),
                (it.owner_id, &to_s, tu(it.id), &pk, &it.kinds, &it.title, now),
            ),
        ).await?;
        for g in it.grant_list() {
            self.batch(
                &[
                    "DELETE FROM items_by_grantee WHERE grantee_key = ? AND status = ? AND item_id = ?",
                    "INSERT INTO items_by_grantee (grantee_key, status, item_id, owner_id, primary_kind, kinds, title, updated_at, via_hospital_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                ],
                (
                    (g.key(), &from_s, tu(it.id)),
                    (g.key(), &to_s, tu(it.id), it.owner_id, &pk, &it.kinds, &it.title, now, g.via_hospital_id),
                ),
            ).await?;
        }
        Ok(())
    }
}

fn summary(id: Uuid, owner: Uuid, pk: Option<String>, kinds: Option<HashSet<String>>, title: Option<String>, updated: Option<CqlTimestamp>, status: ItemStatus) -> ItemSummary {
    let primary_kind = pk.as_deref().and_then(parse).unwrap_or(PrimaryKind::Report);
    ItemSummary {
        id,
        owner_id: owner,
        primary_kind,
        kinds: kinds.unwrap_or_default(),
        title: title.unwrap_or_else(|| primary_kind.label().to_string()),
        status,
        updated_at: updated.unwrap_or(CqlTimestamp(0)),
    }
}
