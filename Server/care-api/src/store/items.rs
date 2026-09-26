//! items, items_by_owner, items_by_grantee, grants_by_id, grants_by_owner (doc 7.2 / 7.3)

use super::Db;
use crate::model::*;
use anyhow::{anyhow, Result};
use scylla::frame::value::{CqlTimestamp, CqlTimeuuid};
use std::collections::HashSet;
use uuid::Uuid;

/// Full item as stored in `items`.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub created_by: Uuid,
    pub created_by_name: String,
    pub date: CqlTimestamp,
    pub item_type: CoreItemType,
    pub title: String,
    pub subtitle: String,
    pub keywords: HashSet<String>,
    pub attachments: Vec<AttachmentUdt>,
    pub links: Vec<String>,
    pub grants: Vec<GrantUdt>,
    pub pointer_to_message: Option<Uuid>,
    pub pointer_item_id: Option<Uuid>,
    pub rating: Option<i8>,
    pub status: ItemStatus,
}

impl Item {
    pub fn grant_list(&self) -> Vec<Grant> { self.grants.iter().filter_map(Grant::from_udt).collect() }
    pub fn facts(&self) -> crate::policy::ItemFacts {
        crate::policy::ItemFacts {
            owner_id: self.owner_id,
            created_by: self.created_by,
            item_type: self.item_type,
            grants: self.grant_list().iter().map(|g| (g.grantee_type, g.grantee_id)).collect(),
        }
    }
}

#[derive(scylla::macros::FromRow)]
struct ItemRow {
    item_id: CqlTimeuuid,
    owner_id: Uuid,
    created_by: Option<Uuid>,
    created_by_name: Option<String>,
    date: Option<CqlTimestamp>,
    core_item_type: Option<String>,
    title: Option<String>,
    subtitle: Option<String>,
    keywords: Option<HashSet<String>>,
    core_item_data: Option<Vec<AttachmentUdt>>,
    links: Option<Vec<String>>,
    access_list: Option<Vec<GrantUdt>>,
    pointer_to_message: Option<CqlTimeuuid>,
    pointer_item_id: Option<CqlTimeuuid>,
    rating: Option<i8>,
    status: Option<String>,
}

impl From<ItemRow> for Item {
    fn from(r: ItemRow) -> Self {
        let item_type = r.core_item_type.as_deref().and_then(parse).unwrap_or(CoreItemType::Report);
        Item {
            id: un(r.item_id),
            owner_id: r.owner_id,
            created_by: r.created_by.unwrap_or(r.owner_id),
            created_by_name: r.created_by_name.unwrap_or_default(),
            date: r.date.unwrap_or(CqlTimestamp(0)),
            item_type,
            title: r.title.unwrap_or_else(|| item_type.label()),
            subtitle: r.subtitle.unwrap_or_default(),
            keywords: r.keywords.unwrap_or_default(),
            attachments: r.core_item_data.unwrap_or_default(),
            links: r.links.unwrap_or_default(),
            grants: r.access_list.unwrap_or_default(),
            pointer_to_message: r.pointer_to_message.map(un),
            pointer_item_id: r.pointer_item_id.map(un),
            rating: r.rating,
            status: r.status.as_deref().and_then(parse).unwrap_or(ItemStatus::Open),
        }
    }
}

/// Row of a list partition — enough for list screens without loading the full item.
#[derive(Clone, Debug)]
pub struct ItemSummary {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub item_type: CoreItemType,
    pub date: CqlTimestamp,
    pub title: String,
    pub subtitle: String,
    pub status: ItemStatus,
}

const ITEM_COLS_Q: &str = "SELECT item_id, owner_id, created_by, created_by_name, date, core_item_type, title, subtitle, keywords, core_item_data, links, access_list, pointer_to_message, pointer_item_id, rating, status FROM items WHERE item_id = ?";

impl Db {
    pub async fn item(&self, id: Uuid) -> Result<Option<Item>> {
        Ok(self.one::<ItemRow>(ITEM_COLS_Q, (tu(id),)).await?.map(Item::from))
    }

    /// Writes the item, its owner row, and one grantee row + grant index rows per grant.
    pub async fn insert_item(&self, it: &Item) -> Result<()> {
        let st = text(&it.status);
        let ty = text(&it.item_type);
        self.exec(
            "INSERT INTO items (item_id, owner_id, created_by, created_by_name, date, core_item_type, title, subtitle, keywords, core_item_data, links, access_list, pointer_to_message, pointer_item_id, rating, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                tu(it.id), it.owner_id, it.created_by, &it.created_by_name, it.date, &ty, &it.title, &it.subtitle,
                &it.keywords, &it.attachments, &it.links, &it.grants, it.pointer_to_message.map(tu),
                it.pointer_item_id.map(tu), it.rating, &st,
            ),
        ).await?;
        // `updated_at` is set separately: SerializeRow tuples stop at 16 values.
        self.exec("UPDATE items SET updated_at = ? WHERE item_id = ?", (now_ts(), tu(it.id))).await?;
        self.exec(
            "INSERT INTO items_by_owner (owner_id, status, item_id, core_item_type, date, title, subtitle) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (it.owner_id, &st, tu(it.id), &ty, it.date, &it.title, &it.subtitle),
        ).await?;
        for g in &it.grants {
            self.index_grant(it, g).await?;
        }
        Ok(())
    }

    async fn index_grant(&self, it: &Item, g: &GrantUdt) -> Result<()> {
        let grant = Grant::from_udt(g).ok_or_else(|| anyhow!("incomplete grant"))?;
        let (st, ty) = (text(&it.status), text(&it.item_type));
        self.batch(
            &[
                "INSERT INTO items_by_grantee (grantee_key, status, item_id, owner_id, core_item_type, date, title, subtitle, via_hospital_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                "INSERT INTO grants_by_id (grant_id, item_id, owner_id, grantee_key) VALUES (?, ?, ?, ?)",
                "INSERT INTO grants_by_owner (owner_id, grant_id, item_id, item_title, core_item_type, grantee_type, grantee_id, grantee_name) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            ],
            (
                (grant.key(), &st, tu(it.id), it.owner_id, &ty, it.date, &it.title, &it.subtitle, grant.via_hospital_id),
                (grant.grant_id, tu(it.id), it.owner_id, grant.key()),
                (it.owner_id, grant.grant_id, tu(it.id), &it.title, &ty, text(&grant.grantee_type), grant.grantee_id, &grant.grantee_name),
            ),
        ).await
    }

    /// Adds a grant to the item's access list and indexes it (doc 7.3 "re-share").
    pub async fn add_grant(&self, it: &Item, g: GrantUdt) -> Result<()> {
        self.exec("UPDATE items SET access_list = access_list + ?, updated_at = ? WHERE item_id = ?", (vec![g.clone()], now_ts(), tu(it.id))).await?;
        self.index_grant(it, &g).await
    }

    /// Revoke: remove from access_list and delete every index row in one logged batch.
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
            "SELECT grant_id, item_id, item_title, core_item_type, grantee_type, grantee_id, grantee_name FROM grants_by_owner WHERE owner_id = ?",
            (owner,)).await?;
        Ok(rows.into_iter().map(|r| OwnedGrantDto {
            grant_id: r.0,
            item_id: un(r.1),
            item_title: r.2.unwrap_or_default(),
            core_item_type: r.3.as_deref().and_then(parse).unwrap_or(CoreItemType::Report),
            grantee_type: r.4.as_deref().and_then(parse).unwrap_or(GranteeType::User),
            grantee_id: r.5,
            grantee_name: r.6.unwrap_or_default(),
        }).collect())
    }

    pub async fn owner_partition(&self, owner: Uuid, status: ItemStatus, limit: i32) -> Result<Vec<ItemSummary>> {
        let rows = self.rows::<(CqlTimeuuid, Option<String>, Option<CqlTimestamp>, Option<String>, Option<String>)>(
            "SELECT item_id, core_item_type, date, title, subtitle FROM items_by_owner WHERE owner_id = ? AND status = ? LIMIT ?",
            (owner, text(&status), limit)).await?;
        Ok(rows.into_iter().map(|r| summary(un(r.0), owner, r.1, r.2, r.3, r.4, status)).collect())
    }

    pub async fn grantee_partition(&self, key: &str, status: ItemStatus, limit: i32) -> Result<Vec<ItemSummary>> {
        let rows = self.rows::<(CqlTimeuuid, Uuid, Option<String>, Option<CqlTimestamp>, Option<String>, Option<String>)>(
            "SELECT item_id, owner_id, core_item_type, date, title, subtitle FROM items_by_grantee WHERE grantee_key = ? AND status = ? LIMIT ?",
            (key, text(&status), limit)).await?;
        Ok(rows.into_iter().map(|r| summary(un(r.0), r.1, r.2, r.3, r.4, r.5, status)).collect())
    }

    /// OPEN ↔ CLOSED moves rows between status partitions (doc 7.3).
    pub async fn set_status(&self, it: &Item, to: ItemStatus) -> Result<()> {
        if it.status == to { return Ok(()); }
        let (from_s, to_s, ty) = (text(&it.status), text(&to), text(&it.item_type));
        self.batch(
            &[
                "UPDATE items SET status = ?, updated_at = ? WHERE item_id = ?",
                "DELETE FROM items_by_owner WHERE owner_id = ? AND status = ? AND item_id = ?",
                "INSERT INTO items_by_owner (owner_id, status, item_id, core_item_type, date, title, subtitle) VALUES (?, ?, ?, ?, ?, ?, ?)",
            ],
            (
                (&to_s, now_ts(), tu(it.id)),
                (it.owner_id, &from_s, tu(it.id)),
                (it.owner_id, &to_s, tu(it.id), &ty, it.date, &it.title, &it.subtitle),
            ),
        ).await?;
        for g in it.grant_list() {
            self.batch(
                &[
                    "DELETE FROM items_by_grantee WHERE grantee_key = ? AND status = ? AND item_id = ?",
                    "INSERT INTO items_by_grantee (grantee_key, status, item_id, owner_id, core_item_type, date, title, subtitle, via_hospital_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                ],
                (
                    (g.key(), &from_s, tu(it.id)),
                    (g.key(), &to_s, tu(it.id), it.owner_id, &ty, it.date, &it.title, &it.subtitle, g.via_hospital_id),
                ),
            ).await?;
        }
        Ok(())
    }

    pub async fn set_rating_keywords(&self, id: Uuid, rating: Option<i8>, keywords: Option<&HashSet<String>>) -> Result<()> {
        if let Some(r) = rating {
            self.exec("UPDATE items SET rating = ?, updated_at = ? WHERE item_id = ?", (r, now_ts(), tu(id))).await?;
        }
        if let Some(k) = keywords {
            self.exec("UPDATE items SET keywords = ?, updated_at = ? WHERE item_id = ?", (k, now_ts(), tu(id))).await?;
        }
        Ok(())
    }

    /// Used by the media worker after thumbnails and page counts are computed (doc 7.4).
    pub async fn set_attachments(&self, id: Uuid, attachments: &[AttachmentUdt]) -> Result<()> {
        self.exec("UPDATE items SET core_item_data = ?, updated_at = ? WHERE item_id = ?", (attachments.to_vec(), now_ts(), tu(id))).await?;
        Ok(())
    }
}

fn summary(id: Uuid, owner: Uuid, ty: Option<String>, date: Option<CqlTimestamp>, title: Option<String>, sub: Option<String>, status: ItemStatus) -> ItemSummary {
    let item_type = ty.as_deref().and_then(parse).unwrap_or(CoreItemType::Report);
    ItemSummary {
        id,
        owner_id: owner,
        item_type,
        date: date.unwrap_or(CqlTimestamp(0)),
        title: title.unwrap_or_else(|| item_type.label()),
        subtitle: sub.unwrap_or_default(),
        status,
    }
}
