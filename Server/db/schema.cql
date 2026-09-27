-- Healoo / CareConnect schema for Cassandra 5.0 (design doc 7.2, extended).
-- "v0.2" = additions for the 0.2 apps; "v2" = DataItem v2 (Documentation/DataItem_Design.md).
-- Apply with: cqlsh -f db/schema.cql
-- Upgrading a trial database from v0.1: docker compose down -v, then up again (test data only).

CREATE KEYSPACE IF NOT EXISTS careconnect
  WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};
USE careconnect;

CREATE TYPE IF NOT EXISTS item_grant (         -- doc name 'grant' is a reserved CQL word
  grant_id uuid,                              -- v0.2: revocable by id (DELETE /v1/grants/{id})
  grantee_type text, grantee_id uuid,
  grantee_name text,                          -- v0.2: shown on Active sharing without a lookup
  via_hospital_id uuid, granted_by uuid, granted_at timestamp
);

-- v2: closing a DataItem (Documentation/DataItem_Design.md 3.2)
CREATE TYPE IF NOT EXISTS item_closure (
  closed_at timestamp, closed_by uuid, feedback text, rating tinyint
);

-- v2: one changed visit of a recurring appointment
CREATE TYPE IF NOT EXISTS visit_exception (
  visit_date text, action text, new_date text, new_time text   -- 'YYYY-MM-DD', 'HH:MM'
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
  employer_doctor_id uuid,                    -- v0.2: assistants -> their doctor (rule 7)
  created_at timestamp,
  email text,                                 -- v0.3: set when an administrator creates the account
  status text,                                -- v0.3: null/ACTIVE or DEACTIVATED
  created_by uuid, deactivated_at timestamp, deactivated_by uuid);   -- v0.3

-- v0.3: upgrade an existing trial database in place (safe to run again).
ALTER TABLE users ADD IF NOT EXISTS email text;
ALTER TABLE users ADD IF NOT EXISTS status text;
ALTER TABLE users ADD IF NOT EXISTS created_by uuid;
ALTER TABLE users ADD IF NOT EXISTS deactivated_at timestamp;
ALTER TABLE users ADD IF NOT EXISTS deactivated_by uuid;

CREATE TABLE IF NOT EXISTS users_by_email (    -- v0.3: admin-created account -> first Auth0 sign-in
  email text PRIMARY KEY, user_id uuid);

CREATE TABLE IF NOT EXISTS users_by_sub (      -- v0.2: Auth0 sub -> user (first-login provisioning)
  auth0_sub text PRIMARY KEY, user_id uuid);

CREATE TABLE IF NOT EXISTS users_by_public_id (
  public_id text PRIMARY KEY,
  user_id uuid, display_name text, roles set<text>, photo_uri text, location text);

-- ---------- DataItem v2: header + child tables ----------
-- The header only. Children (messages, attachments, appointments, alerts) have their own tables
-- so they can grow without limit and be added concurrently.
CREATE TABLE IF NOT EXISTS items (
  item_id timeuuid PRIMARY KEY,
  owner_id uuid,
  primary_kind text,                          -- APPOINTMENT | MESSAGE | ALERT | REPORT (fixed)
  kinds set<text>,                            -- parts the item contains now
  title text,
  keywords set<text>,
  access_list list<frozen<item_grant>>,
  status text,                                -- OPEN | CLOSED
  closure frozen<item_closure>,
  has_report_files boolean,                   -- drives access rule 2
  links list<text>,                           -- https links of a report
  created_by uuid, created_by_name text,
  created_at timestamp, updated_at timestamp);

CREATE TABLE IF NOT EXISTS items_by_owner (
  owner_id uuid, status text, item_id timeuuid,
  primary_kind text, kinds set<text>, title text, updated_at timestamp,
  PRIMARY KEY ((owner_id, status), item_id)) WITH CLUSTERING ORDER BY (item_id DESC);

CREATE TABLE IF NOT EXISTS items_by_grantee (
  grantee_key text,                           -- 'user:<uuid>' or 'hospital:<uuid>'
  status text, item_id timeuuid, owner_id uuid,
  primary_kind text, kinds set<text>, title text, updated_at timestamp,
  via_hospital_id uuid,
  PRIMARY KEY ((grantee_key, status), item_id)) WITH CLUSTERING ORDER BY (item_id DESC);

CREATE TABLE IF NOT EXISTS grants_by_id (      -- revoke by grant id
  grant_id uuid PRIMARY KEY, item_id timeuuid, owner_id uuid, grantee_key text);

CREATE TABLE IF NOT EXISTS grants_by_owner (   -- Active sharing screen
  owner_id uuid, grant_id uuid, item_id timeuuid, item_title text, primary_kind text,
  grantee_type text, grantee_id uuid, grantee_name text,
  PRIMARY KEY ((owner_id), grant_id));

CREATE TABLE IF NOT EXISTS item_messages (
  item_id timeuuid, bucket text,              -- 'YYYY-MM' bounds partition size
  message_id timeuuid, sender_id uuid, body text, client_msg_id text,
  attachment_ids list<uuid>,
  PRIMARY KEY ((item_id, bucket), message_id)) WITH CLUSTERING ORDER BY (message_id DESC);

CREATE TABLE IF NOT EXISTS item_message_dedupe (  -- client_msg_id idempotency
  item_id timeuuid, client_msg_id text, message_id timeuuid,
  PRIMARY KEY ((item_id), client_msg_id)) WITH default_time_to_live = 86400;

CREATE TABLE IF NOT EXISTS item_attachments (
  item_id timeuuid, position int, attachment_id uuid,
  kind text, uri text, mime text, size bigint, name text,
  page_count int, thumb_uri text, sha256 text,
  added_by uuid, added_at timestamp, message_id timeuuid, is_report boolean,
  PRIMARY KEY ((item_id), position));

CREATE TABLE IF NOT EXISTS item_appointments (
  item_id timeuuid, appointment_id uuid,
  patient_id uuid, doctor_id uuid, hospital_id uuid,
  start_date text, start_time text,          -- 'YYYY-MM-DD', 'HH:MM' in the appointment's timezone
  timezone text, duration_min int, notes text,
  frequency text, period text,                -- null frequency = single appointment
  exceptions list<frozen<visit_exception>>,
  status text,                                -- SCHEDULED | COMPLETED | CANCELLED
  visit_status map<text, text>,               -- visit date -> COMPLETED | NO_SHOW
  PRIMARY KEY ((item_id), appointment_id));

-- Calendars: one row per visit in the next 6 months, rebuilt whenever a series changes.
CREATE TABLE IF NOT EXISTS visits_by_doctor (
  doctor_id uuid, day text, starts_at timestamp, appointment_id uuid,
  item_id timeuuid, patient_id uuid, status text,
  PRIMARY KEY ((doctor_id, day), starts_at, appointment_id));

CREATE TABLE IF NOT EXISTS visits_by_patient (
  patient_id uuid, month text, starts_at timestamp, appointment_id uuid,
  item_id timeuuid, doctor_id uuid, status text,
  PRIMARY KEY ((patient_id, month), starts_at, appointment_id));

CREATE TABLE IF NOT EXISTS item_alerts (
  item_id timeuuid, alert_id uuid,
  type text, text text, for_user uuid,
  fires_at timestamp, timezone text, frequency text, period text,
  appointment_id uuid, active boolean,
  PRIMARY KEY ((item_id), alert_id));

-- Read by the alert scheduler (care-worker) every minute.
CREATE TABLE IF NOT EXISTS alerts_due (
  due_minute timestamp, alert_id uuid, item_id timeuuid, for_user uuid,
  PRIMARY KEY ((due_minute), alert_id)) WITH default_time_to_live = 604800;

-- Messages tab: items with a discussion, per person (replaces v0.1 chat threads).
CREATE TABLE IF NOT EXISTS conversations_by_user (
  user_id uuid, other_user_id uuid, item_id timeuuid,
  last_message text, last_message_at timestamp, unread int,
  PRIMARY KEY ((user_id), other_user_id, item_id));

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

CREATE TABLE IF NOT EXISTS connections_by_user (
  user_id uuid, other_role text, other_id uuid, other_name text, via text,
  PRIMARY KEY ((user_id), other_role, other_id));

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
