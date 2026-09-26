-- Healoo / CareConnect schema for Cassandra 5.0 (design doc 7.2, extended for the v0.2 apps).
-- Changes from the doc are marked "-- v0.2". Apply with: cqlsh -f db/schema.cql

CREATE KEYSPACE IF NOT EXISTS careconnect
  WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};
USE careconnect;

-- kind: IMAGE | PDF
CREATE TYPE IF NOT EXISTS attachment (
  kind text, uri text, mime text, size bigint, sha256 text, position int,
  page_count int, thumb_uri text,
  name text                                   -- v0.2: original file name shown in the viewer
);

CREATE TYPE IF NOT EXISTS item_grant (         -- doc name 'grant' is a reserved CQL word
  grant_id uuid,                              -- v0.2: revocable by id (DELETE /v1/grants/{id})
  grantee_type text, grantee_id uuid,
  grantee_name text,                          -- v0.2: shown on Active sharing without a lookup
  via_hospital_id uuid, granted_by uuid, granted_at timestamp
);

-- ---------- users ----------
CREATE TABLE IF NOT EXISTS users (
  user_id uuid PRIMARY KEY,
  auth0_sub text,
  public_id text,                             -- v0.2
  roles set<text>,
  display_name text,
  name_tokens set<text>,                      -- v0.2: lower-case words for name search
  headline text,                              -- v0.2
  photo_uri text, location text, phone text, official_number text,
  primary_hospital_id uuid,
  employer_doctor_id uuid,                    -- v0.2: assistants → their doctor (rule 7)
  created_at timestamp);

CREATE TABLE IF NOT EXISTS users_by_sub (      -- v0.2: Auth0 sub → user (first-login provisioning)
  auth0_sub text PRIMARY KEY, user_id uuid);

CREATE TABLE IF NOT EXISTS users_by_public_id (
  public_id text PRIMARY KEY,
  user_id uuid, display_name text, roles set<text>, photo_uri text, location text);

-- ---------- items ----------
CREATE TABLE IF NOT EXISTS items (
  item_id timeuuid PRIMARY KEY, owner_id uuid, created_by uuid,
  created_by_name text,                       -- v0.2
  date timestamp, core_item_type text,
  title text, subtitle text,                  -- v0.2
  keywords set<text>, core_item_data list<frozen<attachment>>,
  links list<text>,                           -- doc 2.2
  access_list list<frozen<item_grant>>,            -- v0.2: list (append/remove by value) instead of set
  pointer_to_message timeuuid, pointer_item_id timeuuid, pointer_item_type text,
  rating tinyint, status text, updated_at timestamp);

CREATE TABLE IF NOT EXISTS items_by_owner (
  owner_id uuid, status text, item_id timeuuid, core_item_type text, date timestamp,
  keywords set<text>, title text,
  subtitle text,                              -- v0.2
  PRIMARY KEY ((owner_id, status), item_id)) WITH CLUSTERING ORDER BY (item_id DESC);

CREATE TABLE IF NOT EXISTS items_by_grantee (
  grantee_key text,                           -- 'user:<uuid>' or 'hospital:<uuid>'
  status text, item_id timeuuid, owner_id uuid, core_item_type text, date timestamp,
  title text, subtitle text,                  -- v0.2
  via_hospital_id uuid,
  PRIMARY KEY ((grantee_key, status), item_id)) WITH CLUSTERING ORDER BY (item_id DESC);

CREATE TABLE IF NOT EXISTS grants_by_id (      -- v0.2: revoke by grant id
  grant_id uuid PRIMARY KEY, item_id timeuuid, owner_id uuid, grantee_key text);

CREATE TABLE IF NOT EXISTS grants_by_owner (   -- v0.2: Active sharing screen
  owner_id uuid, grant_id uuid, item_id timeuuid, item_title text, core_item_type text,
  grantee_type text, grantee_id uuid, grantee_name text,
  PRIMARY KEY ((owner_id), grant_id));

-- ---------- organisation ----------
CREATE TABLE IF NOT EXISTS affiliations_by_doctor (
  doctor_id uuid, hospital_id uuid, role text, started_at timestamp, ended_at timestamp,
  PRIMARY KEY (doctor_id, hospital_id));

CREATE TABLE IF NOT EXISTS affiliations_by_hospital (
  hospital_id uuid, doctor_id uuid, started_at timestamp, ended_at timestamp,
  PRIMARY KEY (hospital_id, doctor_id));

CREATE TABLE IF NOT EXISTS delegations_by_assistant (
  assistant_id uuid, doctor_id uuid, patient_id uuid, granted_at timestamp,
  PRIMARY KEY ((assistant_id), doctor_id, patient_id));

-- ---------- messaging ----------
CREATE TABLE IF NOT EXISTS threads_by_user (
  user_id uuid, other_user_id uuid, thread_id timeuuid,
  last_message text,                          -- v0.2: preview in thread lists
  last_message_at timestamp, unread int,
  PRIMARY KEY ((user_id), other_user_id, thread_id));

CREATE TABLE IF NOT EXISTS threads_by_id (     -- v0.2: membership check
  thread_id timeuuid PRIMARY KEY, user_a uuid, user_b uuid, created_at timestamp);

CREATE TABLE IF NOT EXISTS messages_by_thread (
  thread_id timeuuid, bucket text,            -- 'YYYY-MM' bounds partition size
  message_id timeuuid, sender_id uuid, body text, attachments list<frozen<attachment>>,
  client_msg_id text, linked_item_id timeuuid,
  PRIMARY KEY ((thread_id, bucket), message_id)) WITH CLUSTERING ORDER BY (message_id DESC);

CREATE TABLE IF NOT EXISTS message_dedupe (    -- v0.2: client_msg_id idempotency (doc 4.4)
  thread_id timeuuid, client_msg_id text, message_id timeuuid,
  PRIMARY KEY ((thread_id), client_msg_id)) WITH default_time_to_live = 86400;

CREATE TABLE IF NOT EXISTS connections_by_user (
  user_id uuid, other_role text, other_id uuid, other_name text, via text,
  PRIMARY KEY ((user_id), other_role, other_id));

-- Kept from the doc; the trial API computes the dashboard from list partitions instead.
CREATE TABLE IF NOT EXISTS dashboard_counters (
  user_id uuid, counter_name text, value counter,
  PRIMARY KEY (user_id, counter_name));

-- ---------- devices and settings ----------
CREATE TABLE IF NOT EXISTS devices_by_user (
  user_id uuid, fcm_token text, platform text, updated_at timestamp,
  PRIMARY KEY (user_id, fcm_token)) WITH default_time_to_live = 5184000;  -- 60 days

CREATE TABLE IF NOT EXISTS notification_prefs (  -- v0.2: Settings screen
  user_id uuid PRIMARY KEY, push_messages boolean, push_reports boolean,
  quiet_hours boolean, quiet_start text, quiet_end text);

-- ---------- events ----------
CREATE TABLE IF NOT EXISTS outbox (
  shard int, event_id text, topic text, event_key text, payload text, created_at timestamp,
  PRIMARY KEY ((shard), event_id));

CREATE TABLE IF NOT EXISTS processed_events (  -- consumer idempotency, 1-day TTL (doc 6.3)
  consumer text, event_id text,
  PRIMARY KEY ((consumer), event_id)) WITH default_time_to_live = 86400;

CREATE TABLE IF NOT EXISTS access_audit (      -- written by audit-writer (doc 6.4)
  item_id timeuuid, decided_at timeuuid, user_id uuid, decision text, reason text, event_id text,
  PRIMARY KEY ((item_id), decided_at)) WITH CLUSTERING ORDER BY (decided_at DESC)
  AND default_time_to_live = 7776000;         -- 90 days

-- ---------- search (Storage-Attached Indexes) ----------
CREATE INDEX IF NOT EXISTS users_name_tokens_sai ON users (name_tokens) USING 'sai';
CREATE INDEX IF NOT EXISTS conn_name_sai ON connections_by_user (other_name) USING 'sai';
CREATE INDEX IF NOT EXISTS items_kw_sai ON items (keywords) USING 'sai';
