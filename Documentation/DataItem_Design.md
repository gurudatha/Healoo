# DataItem v2 — design specification

Status: **implemented** in server v0.2 and apps 0.4 (not compiled yet; see "Implementation notes"
at the end). Replaces the DataItem in design document section 2.2 and the `items` shape used by
server v0.1 and apps 0.3. The API reference is `userapidocumentation.txt` sections 5–9.

## 1. Idea in one paragraph

A DataItem is a **container for one case**. It is created from exactly one primary part — an
appointment, a message, an alert or a report — and can then grow: people discuss it (messages),
add files (attachments), book visits (appointments) and set reminders (alerts). It has one owner
(the patient), one access list that covers everything inside it, and one status. When it is
closed, whoever closes it may leave feedback, and the patient may give a rating.

```
DataItem (header)
 ├── owner, primary_kind, title, keywords, status, access list
 ├── closure        0..1   feedback (optional), rating (optional, patient only)
 ├── messages       0..n
 ├── attachments    0..n
 ├── appointments   0..n   each may recur
 └── alerts         0..n
```

## 2. Decisions

| # | Decision | Why |
| --- | --- | --- |
| D1 | `primary_kind` is fixed at creation: `APPOINTMENT`, `MESSAGE`, `ALERT` or `REPORT`. | Keeps the icon, default title and list filters stable as the item grows. |
| D2 | `kinds` is derived: the set of part types the item currently contains. | Lists can show "report + 2 messages + appointment" and filter on any part. |
| D3 | One access list per item; every child inherits it. | One policy check for everything inside the item; no per-message sharing mistakes. |
| D4 | Children are stored in their own tables, not inside the item row. | Messages and recurring visits are unbounded; separate rows avoid oversized rows and lost updates when two people add at once. The API still returns them together. |
| D5 | **Every message belongs to an item.** Starting a new conversation creates a `MESSAGE` item. | Discussions stay attached to what they are about; access rules cover messages. Replaces the per-pair chat threads of v0.1. |
| D6 | Alerts: 0..n per item (the app shows one "Add alert" action). | Appointments create their own reminder alerts, and a case may also need a medication reminder. |
| D7 | Feedback and rating are part of closing, not item types. | Matches the requirement; `FEEDBACK` is removed as a type. |
| D8 | `PAYMENT` is removed as a primary kind. | Not in the requirement; can return later as another child type. |
| D9 | Appointments store a recurrence rule plus exceptions, not one row per visit. | "Weekly for 3 months" is one record; single visits can still be moved or cancelled. |

## 3. Fields

M = mandatory, O = optional, D = derived by the server.

### 3.1 DataItem

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `item_id` | D | timeuuid | |
| `owner_id` | M | uuid | The patient. Must have role PATIENT. |
| `primary_kind` | M | enum | `APPOINTMENT` \| `MESSAGE` \| `ALERT` \| `REPORT` (D1) |
| `kinds` | D | set of enum | Adds `MESSAGE`, `ATTACHMENT`, `APPOINTMENT`, `ALERT`, `REPORT` as parts are added (D2) |
| `title` | O | text ≤ 120 | Default from the primary part (report title, "Appointment with Dr. …", first line of message, alert text) |
| `keywords` | O | set of text, ≤ 20 | |
| `access_list` | M | list of Grant | Owner always has access even if the list is empty (see 3.8) |
| `status` | M | enum | `OPEN` \| `CLOSED` |
| `closure` | O | Closure | Present only when `status = CLOSED` |
| `created_by`, `created_at`, `updated_at` | D | | |
| `counts` | D | object | `{ messages, attachments, appointments, alerts, unread_messages }` for list screens |

### 3.2 Closure

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `closed_at`, `closed_by` | D | | |
| `feedback` | O | text ≤ 1000 | By whoever closes (patient or clinician with full access) |
| `rating` | O | int 1–5 | **Only accepted when the owner (patient) closes.** Rejected (400) from anyone else. |

Reopening (`status → OPEN`) keeps the previous closure in history (`closure_history`, not
returned in normal reads) and clears `closure`.

### 3.3 Message

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `message_id` | D | timeuuid | |
| `sender_id` | D | uuid | Must have full access to the item |
| `body` | M | text 1–4000 | |
| `client_msg_id` | M | text ≤ 64 | Idempotency (resend returns the first message) |
| `attachment_ids` | O | list of uuid | Files posted with the message; they also appear in the item's attachments |
| `sent_at` | D | timestamp | |

### 3.4 Attachment

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `attachment_id` | D | uuid | |
| `kind` | M | enum | `IMAGE` \| `PDF` |
| `uri` | M | text | `obj:` key from `/v1/uploads/presign`; returned as a 10-minute signed URL |
| `mime`, `size`, `name` | M | | Limits unchanged: images ≤ 10 MB, PDFs ≤ 25 MB, ≤ 20 per request |
| `position` | D | int | Display order within the item |
| `page_count`, `thumb_uri`, `sha256` | D | | Filled by the media worker |
| `added_by`, `added_at` | D | | |
| `message_id` | O | timeuuid | Set when posted inside a message |
| `is_report` | O | bool | True for files that make up a report (drives rule 2 in 3.8) |

### 3.5 Alert

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `alert_id` | D | uuid | |
| `type` | M | enum | `MEDICATION` \| `APPOINTMENT_REMINDER` \| `FOLLOW_UP` \| `RESULT_READY` \| `CUSTOM` |
| `text` | M | text ≤ 200 | |
| `for_user` | M | uuid | Must have access to the item |
| `fires_at` | M | date + time + timezone | First (or only) time |
| `recurrence` | O | Recurrence | Same shape as appointments (3.7) |
| `appointment_id` | O | uuid | Set for reminders created by an appointment |
| `active` | D | bool | False after the last firing, when cancelled, or when the item closes |

### 3.6 Appointment

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `appointment_id` | D | uuid | |
| `patient_id` | M | uuid | "User". Must equal the item's `owner_id`. |
| `doctor_id` | M | uuid | Must have role DOCTOR. Gets access to the item automatically (3.8). |
| `date` | M | `YYYY-MM-DD` | First visit |
| `time` | M | `HH:MM` (24 h) | |
| `timezone` | D/O | IANA name | Default `Asia/Kolkata` (the creator's device zone if sent) |
| `duration_min` | O | int 5–240 | Default 15 |
| `hospital_id` | O | uuid | Where; must be a HOSPITAL the doctor is affiliated with |
| `notes` | O | text ≤ 500 | |
| `recurrence` | O | Recurrence | See 3.7 |
| `exceptions` | O | list | Per-visit changes: `{ date, action: CANCELLED \| MOVED, new_date?, new_time? }` |
| `status` | D | enum | Series: `SCHEDULED` \| `COMPLETED` \| `CANCELLED` |
| `visits` | D | list | Returned on read: the next visits with their own status `SCHEDULED \| COMPLETED \| CANCELLED \| NO_SHOW` |

### 3.7 Recurrence

| Field | | Type | Notes |
| --- | --- | --- | --- |
| `frequency` | M (if recurring) | enum | `DAILY` \| `WEEKLY` \| `BIWEEKLY` \| `MONTHLY` \| `QUARTERLY` |
| `period` | O | enum | `ONE_MONTH` \| `TWO_MONTHS` \| `THREE_MONTHS` \| `SIX_MONTHS` |

Rules:

1. Visits are the first `date`+`time`, then every step (1 day, 7 days, 14 days, 1 month,
   3 months) while the visit date is **before** `date + period`.
2. `MONTHLY`/`QUARTERLY` keep the day of month; if it doesn't exist (29–31), use the month's
   last day.
3. A combination that yields only one visit is rejected (400): `MONTHLY` + `ONE_MONTH`,
   `QUARTERLY` + `ONE_MONTH`/`TWO_MONTHS`/`THREE_MONTHS`. Book a single appointment instead.
4. `period` omitted: the series continues until cancelled or the item is closed. **`DAILY`
   requires a period** (to avoid endless daily visits).
5. Closing the item cancels all future visits and deactivates alerts.
6. Changing the rule applies to future visits only; past visits keep their status.

Visit counts for a start on 1 Oct:

| | 1 month | 2 months | 3 months | 6 months | none |
| --- | --- | --- | --- | --- | --- |
| Daily | 31 | 61 | 92 | 182 | rejected |
| Weekly | 5 | 9 | 14 | 26 | until cancelled |
| Bi-weekly | 3 | 5 | 7 | 13 | until cancelled |
| Monthly | rejected | 2 | 3 | 6 | until cancelled |
| Quarterly | rejected | rejected | rejected | 2 | until cancelled |

Each visit creates `APPOINTMENT_REMINDER` alerts for the patient (1 day and 1 hour before) and
for the doctor (1 hour before), unless the patient turns them off.

### 3.8 Access (design doc 2.3, applied to the container)

- The item's access list decides access to the **whole item**; children have no own lists.
- Decisions are unchanged (owner, direct grant, hospital grant with active affiliation,
  assistant delegation). What each level sees:

| Access | Header, keywords, status | Appointments | Messages | Attachments | Alerts |
| --- | --- | --- | --- | --- | --- |
| Full | ✓ | ✓ | ✓ read + write | ✓ | own alerts |
| Metadata only | ✓ | date/time/doctor only | ✗ | ✗ | ✗ |
| Deny | ✗ | ✗ | ✗ | ✗ | ✗ |

- Rule 2 (reports readable by doctors only): if the item contains report files
  (`is_report`), a non-doctor grantee gets **metadata only** for the whole item.
- Booking an appointment grants the doctor (user grant). Uploading for a patient grants the
  uploader, as today. Only the owner shares further or revokes.
- Writing (message, attachment, appointment, alert) needs full access. Closing needs full
  access; rating needs to be the owner.

## 4. API

| Method | Path | Body / notes |
| --- | --- | --- |
| POST | `/v1/items` | `owner_id?`, `title?`, `keywords?`, `share_with?` and **exactly one** of `appointment`, `message`, `alert`, `report` (`report` = `{ title, date, attachments[], links[] }`) |
| GET | `/v1/items?status=&kind=&limit=` | Headers + `counts`; `kind` matches any part |
| GET | `/v1/items/{id}` | Header + first page of each child list |
| PATCH | `/v1/items/{id}` | `title`, `keywords` (owner) |
| POST | `/v1/items/{id}/close` | `{ feedback?, rating? }` |
| POST | `/v1/items/{id}/reopen` | |
| GET/POST | `/v1/items/{id}/messages` | Paged (`page_state`); POST `{ body, client_msg_id, attachment_ids? }` |
| GET/POST | `/v1/items/{id}/attachments` | POST `{ attachments[], is_report? }` |
| GET/POST | `/v1/items/{id}/appointments` | POST Appointment (3.6) |
| PATCH | `/v1/items/{id}/appointments/{aid}` | Change rule/time (future visits), `status: CANCELLED` |
| POST | `/v1/items/{id}/appointments/{aid}/visits/{date}` | `{ action: CANCELLED \| MOVED \| COMPLETED \| NO_SHOW, new_date?, new_time? }` |
| GET/POST/DELETE | `/v1/items/{id}/alerts[/{alert_id}]` | |
| GET | `/v1/appointments?from=&to=` | Calendar across items (patient: own; doctor: theirs) |
| GET | `/v1/conversations?with=` | Messages tab: items with messages, grouped by the other person (replaces `/v1/threads`) |

WebSocket frames: `message.new {item_id, message}`, `item.updated {item_id, change}` with
`change` in `created | message | attachment | appointment | alert | closed | reopened | shared`.

## 5. Example (full access)

```json
{
  "item_id": "8a4b1c20-7f2e-11f1-9c3d-2b1f5e7a9d01",
  "owner_id": "5eed0000-0000-4000-8000-000000000001",
  "primary_kind": "REPORT",
  "kinds": ["REPORT", "ATTACHMENT", "MESSAGE", "APPOINTMENT", "ALERT"],
  "title": "CBC – Complete blood count",
  "keywords": ["CBC", "Haemoglobin"],
  "status": "OPEN",
  "access_list": [
    { "grant_id": "…", "grantee_type": "USER", "grantee_id": "5eed…0002", "grantee_name": "Dr. Anitha Rao" },
    { "grant_id": "…", "grantee_type": "USER", "grantee_id": "5eed…0004", "grantee_name": "City Diagnostics" }
  ],
  "counts": { "messages": 2, "attachments": 4, "appointments": 1, "alerts": 1, "unread_messages": 1 },
  "attachments": [
    { "attachment_id": "…", "kind": "PDF", "name": "CBC_20Sep2026.pdf", "mime": "application/pdf",
      "size": 1203344, "position": 0, "page_count": 3, "is_report": true, "uri": "https://…signed…" }
  ],
  "messages": [
    { "message_id": "…", "sender_id": "5eed…0002", "body": "Haemoglobin is slightly low. Let's review in two weeks.",
      "sent_at": "2026-09-21T10:14:02+05:30" }
  ],
  "appointments": [
    { "appointment_id": "…", "patient_id": "5eed…0001", "doctor_id": "5eed…0002",
      "date": "2026-10-05", "time": "11:30", "timezone": "Asia/Kolkata", "duration_min": 15,
      "recurrence": { "frequency": "BIWEEKLY", "period": "THREE_MONTHS" },
      "exceptions": [ { "date": "2026-10-19", "action": "MOVED", "new_date": "2026-10-20", "new_time": "10:00" } ],
      "status": "SCHEDULED",
      "visits": [
        { "date": "2026-10-05", "time": "11:30", "status": "SCHEDULED" },
        { "date": "2026-10-20", "time": "10:00", "status": "SCHEDULED" }
      ] }
  ],
  "alerts": [
    { "alert_id": "…", "type": "MEDICATION", "text": "Iron tablet after dinner", "for_user": "5eed…0001",
      "fires_at": "2026-09-21T20:00:00+05:30", "recurrence": { "frequency": "DAILY", "period": "ONE_MONTH" },
      "active": true }
  ],
  "created_by": "5eed…0004", "created_at": "2026-09-20T09:02:11+05:30"
}
```

## 6. Cassandra tables

```sql
-- Header only; children live in their own tables (D4).
CREATE TABLE items (
  item_id timeuuid PRIMARY KEY, owner_id uuid, primary_kind text, kinds set<text>,
  title text, keywords set<text>, access_list list<frozen<item_grant>>, status text,
  closure frozen<item_closure>, created_by uuid, created_at timestamp, updated_at timestamp,
  has_report_files boolean);

CREATE TYPE item_closure (closed_at timestamp, closed_by uuid, feedback text, rating tinyint);

CREATE TABLE item_counts (item_id timeuuid PRIMARY KEY,
  messages counter, attachments counter, appointments counter, alerts counter);

CREATE TABLE item_messages (
  item_id timeuuid, bucket text, message_id timeuuid, sender_id uuid, body text,
  client_msg_id text, attachment_ids list<uuid>,
  PRIMARY KEY ((item_id, bucket), message_id)) WITH CLUSTERING ORDER BY (message_id DESC);

CREATE TABLE item_attachments (
  item_id timeuuid, position int, attachment_id uuid, kind text, uri text, mime text, size bigint,
  name text, page_count int, thumb_uri text, sha256 text, added_by uuid, added_at timestamp,
  message_id timeuuid, is_report boolean,
  PRIMARY KEY ((item_id), position));

CREATE TABLE item_appointments (
  item_id timeuuid, appointment_id uuid, patient_id uuid, doctor_id uuid, hospital_id uuid,
  start_date date, start_time time, timezone text, duration_min int, notes text,
  frequency text, period text, exceptions list<frozen<visit_exception>>, status text,
  PRIMARY KEY ((item_id), appointment_id));

CREATE TYPE visit_exception (visit_date date, action text, new_date date, new_time time);

-- Calendars: one row per visit for the next 6 months, rebuilt when a rule or exception changes.
CREATE TABLE visits_by_doctor (
  doctor_id uuid, day date, starts_at timestamp, appointment_id uuid, item_id timeuuid,
  patient_id uuid, status text,
  PRIMARY KEY ((doctor_id, day), starts_at, appointment_id));
CREATE TABLE visits_by_patient (
  patient_id uuid, month text, starts_at timestamp, appointment_id uuid, item_id timeuuid,
  doctor_id uuid, status text,
  PRIMARY KEY ((patient_id, month), starts_at, appointment_id));

CREATE TABLE item_alerts (
  item_id timeuuid, alert_id uuid, type text, text text, for_user uuid, fires_at timestamp,
  timezone text, frequency text, period text, appointment_id uuid, active boolean,
  PRIMARY KEY ((item_id), alert_id));

-- Read by the alert scheduler every minute.
CREATE TABLE alerts_due (
  due_minute timestamp, alert_id uuid, item_id timeuuid, for_user uuid,
  PRIMARY KEY ((due_minute), alert_id));

-- Messages tab (D5): items with messages per person.
CREATE TABLE conversations_by_user (
  user_id uuid, other_user_id uuid, item_id timeuuid, last_message text,
  last_message_at timestamp, unread int,
  PRIMARY KEY ((user_id), other_user_id, item_id));
```

`items_by_owner`, `items_by_grantee`, `grants_by_*`, users, affiliations and delegations stay
as in server v0.1, with `primary_kind` and `kinds` replacing `core_item_type`.
`threads_by_user`, `threads_by_id`, `messages_by_thread` and `message_dedupe` are replaced by
`item_messages` and `conversations_by_user` (dedupe moves to a `(item_id, client_msg_id)` table).

## 7. Types

Rust (server, `src/model.rs`):

```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimaryKind { Appointment, Message, Alert, Report }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Frequency { Daily, Weekly, Biweekly, Monthly, Quarterly }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Period { OneMonth, TwoMonths, ThreeMonths, SixMonths }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Recurrence { pub frequency: Frequency, pub period: Option<Period> }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Appointment {
    pub appointment_id: Option<Uuid>,
    pub patient_id: Uuid,
    pub doctor_id: Uuid,
    pub date: chrono::NaiveDate,
    pub time: chrono::NaiveTime,
    #[serde(default = "default_tz")] pub timezone: String,
    #[serde(default = "default_duration")] pub duration_min: u16,
    pub hospital_id: Option<Uuid>,
    pub notes: Option<String>,
    pub recurrence: Option<Recurrence>,
    #[serde(default)] pub exceptions: Vec<VisitException>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Closure { pub feedback: Option<String>, pub rating: Option<u8> }

/// POST /v1/items: exactly one primary part.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PrimaryPart {
    Appointment(Appointment),
    Message(NewMessage),
    Alert(NewAlert),
    Report(NewReport),
}
```

Kotlin (Android, `data/Models.kt`):

```kotlin
@Serializable enum class PrimaryKind { APPOINTMENT, MESSAGE, ALERT, REPORT }
@Serializable enum class Frequency { DAILY, WEEKLY, BIWEEKLY, MONTHLY, QUARTERLY }
@Serializable enum class Period { ONE_MONTH, TWO_MONTHS, THREE_MONTHS, SIX_MONTHS }

@Serializable data class Recurrence(val frequency: Frequency, val period: Period? = null)

@Serializable data class Appointment(
    @SerialName("appointment_id") val appointmentId: String? = null,
    @SerialName("patient_id") val patientId: String,
    @SerialName("doctor_id") val doctorId: String,
    val date: String,                      // YYYY-MM-DD
    val time: String,                      // HH:MM
    val timezone: String = "Asia/Kolkata",
    @SerialName("duration_min") val durationMin: Int = 15,
    val recurrence: Recurrence? = null,
    val visits: List<Visit> = emptyList(),
)

@Serializable data class Closure(val feedback: String? = null, val rating: Int? = null)

@Serializable data class DataItem(
    @SerialName("item_id") val itemId: String,
    @SerialName("owner_id") val ownerId: String,
    @SerialName("primary_kind") val primaryKind: PrimaryKind,
    val kinds: Set<String> = emptySet(),
    val title: String,
    val keywords: List<String> = emptyList(),
    val status: ItemStatus,
    val closure: Closure? = null,
    @SerialName("access_list") val accessList: List<Grant> = emptyList(),
    val counts: ItemCounts = ItemCounts(),
    val messages: List<Message> = emptyList(),
    val attachments: List<Attachment> = emptyList(),
    val appointments: List<Appointment> = emptyList(),
    val alerts: List<Alert> = emptyList(),
)
```

## 8. Screens affected

- **Item view:** sections for attachments, discussion, appointments and alerts, each with an
  "Add" action for users with full access; "Close" opens a sheet with feedback and (patient
  only) 1–5 stars.
- **New item:** choose the primary part first (Book appointment, Send message, Set alert,
  Upload report).
- **Booking sheet:** doctor, date, time, "Repeat" (off / daily / weekly / every 2 weeks /
  monthly / every 3 months) and "For" (1 / 2 / 3 / 6 months, or until cancelled). Shows the
  number of visits and rejects the combinations in 3.7.
- **Messages tab:** conversations grouped by person; each opens the item's discussion.
- **Calendar:** new list of upcoming visits from `/v1/appointments`.

## 9. Moving from v0.1

The trial holds test data only, so the simplest path is to apply the new schema and run
`care-seed` again. For reference, the mapping is:

| v0.1 | v2 |
| --- | --- |
| `core_item_type` REPORT / ALERT / BOOKING / MESSAGE | `primary_kind` REPORT / ALERT / APPOINTMENT / MESSAGE |
| `core_item_type` FEEDBACK | `closure.feedback` on the item it pointed to |
| `core_item_type` PAYMENT | dropped (D8) |
| `core_item_data` | `item_attachments` (`is_report` = true for REPORT items) |
| `pointer_item_id` pairs | merged into one item |
| chat thread between two users | one `MESSAGE` item owned by the patient in the pair |
| `rating` | `closure.rating` |

## 10. Open points

- Who may book: implemented as proposed — the patient, the doctor, or the doctor's assistant,
  with the doctor always the appointment's `doctor_id`. Change `may_manage` in
  `src/api/parts.rs` if this should be narrower.
- Payment: add back later as a child (`payments`) once billing is designed.
- Group discussions (patient + two doctors) work automatically when both doctors are on the
  access list; decide whether that is wanted for every item kind.

## 11. Implementation notes (v0.2)

Where the code differs from, or adds detail to, the sections above:

- **Counts** are computed from the lists fetched for the response (no `item_counts` counter
  table). List entries (`GET /v1/items`) carry counts but empty child lists.
- **Messages:** `GET /v1/items/{id}` returns the latest 100; `GET .../messages?limit=` up to 500.
  Paging cursors (`page_state`) are not implemented yet. Message dedupe is the
  `item_message_dedupe` table keyed by `(item_id, client_msg_id)`.
- **Appointment PATCH** replaces the whole rule you send; past visits keep their status.
  `recurrence: null` turns a series into a single visit.
- **Visits** in responses: the next 12 upcoming (`visits`), with `original_date` used to address
  one visit in `POST .../visits/{date}`. `visit_count` is absent when a series runs until
  cancelled. Visit statuses live in `item_appointments.visit_status` (date → COMPLETED/NO_SHOW);
  moves and cancellations are `exceptions`.
- **Calendar tables** `visits_by_doctor` / `visits_by_patient` are rebuilt for the next 6 months
  whenever an appointment or exception changes. Dates are stored as text (`YYYY-MM-DD`).
- **Alerts:** the worker's scheduler runs every 20 seconds from `alerts_due`, sends a push and a
  WebSocket `alert` frame, then schedules the next firing. Cancelled appointment reminders and
  alerts of closed items are skipped.
- **Reopen** does not restore cancelled visits; book again if needed.
- **Old tables** (`threads_*`, `messages_by_thread`, `message_dedupe`, `dashboard_counters`) and
  `/v1/threads` are removed. The trial database must be recreated and re-seeded.
- **Seed data** (`care-seed`): a CBC report that grew into a full case (files, discussion,
  bi-weekly appointment for 3 months, daily medication alert), a BP discussion, a standalone
  appointment, a standalone weekly alert, a closed item with feedback and a 5-star rating, and
  Priya's unshared lipid profile.
