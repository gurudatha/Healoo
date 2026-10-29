# Administration — design specification

Status: **implemented** (server v0.3, apps 0.5). Sections 8–12 (platform dashboard, hospital
profile, encrypted administrators file, invite codes, limits) added in server v0.4. The server is built, unit-tested and checked
against the LAN trial server (section 7). Android compiles. iOS is written but not yet compiled.

## 1. Idea in one paragraph

A **hospital administrator** manages the accounts of their own hospital. They can add a user
(a patient) and add a doctor, who joins the administrator's hospital at once. They can delete a
doctor, which **deactivates** the account rather than erasing it. They can reactivate that doctor
later. They can never delete a user: users own health records. A platform administrator
(APP_ADMINISTRATOR) can do the same for any hospital, naming it with `hospital_id`. In both apps
this is **Settings → Administration**, shown only to administrators.

## 2. Decisions

| # | Decision |
| --- | --- |
| R1 | Administrators create two kinds of account only: **users** (PATIENT) and **doctors** (DOCTOR). Labs, hospitals, assistants and administrators are not created here. |
| R2 | A new doctor works at the administrator's hospital (affiliation + contact with the hospital). Their **designation** (e.g. Cardiologist, Urologist, Paediatrician) is shown in the profile line as "Doctor · Cardiologist". There is no department field yet. |
| R3 | **Delete a doctor = deactivate.** The doctor can't sign in, is hidden from search and hospital doctor lists, and their affiliation ends, so any access they had through the hospital stops at once. Items, messages, appointments and grants they created stay. Deactivation can be undone. |
| R4 | **Users can't be deleted.** `DELETE /v1/admin/users/{id}` always answers 403, and the apps offer no button for it. |
| R5 | A hospital administrator acts **only for their own hospital**: they can't list, deactivate or reactivate another hospital's doctors. |
| R6 | Every account an administrator creates has an **email**, which must be unique. The first Auth0 sign-in with the same verified email is linked to that account instead of creating a new one. |

These make up **access rule 10** (design document section 2.3). Sections 8 and 9 add **rule 11**.

## 3. API

All endpoints need an ADMINISTRATOR or APP_ADMINISTRATOR token. The `hospital_id` field or
query parameter is required for a platform administrator and ignored for a hospital
administrator, who always acts for their own hospital.

| Method | Path | Result |
| --- | --- | --- |
| POST | `/v1/admin/users` | 201, new user (patient) |
| POST | `/v1/admin/doctors` | 201, new doctor at the hospital |
| GET | `/v1/admin/doctors` | The hospital's doctors, deactivated ones last (`active: false`) |
| DELETE | `/v1/admin/doctors/{user_id}` | 204, doctor deactivated (repeating the call is harmless) |
| POST | `/v1/admin/doctors/{user_id}/reactivate` | 200, doctor active again and back at the hospital |
| DELETE | `/v1/admin/users/{user_id}` | Always 403 (R4) |

Request body for both POSTs:

```json
{
  "display_name": "Dr. Kavya Menon",
  "email": "kavya.menon@example.com",
  "designation": "Cardiologist",
  "official_number": "REG-12345",
  "location": "Bengaluru",
  "hospital_id": null
}
```

`display_name` (1–80 characters) and `email` are required. `designation` and
`official_number` apply to doctors only and are ignored for users. The response is the public
profile plus `email` and `active`.

| Status | When |
| --- | --- |
| 400 | Missing name, or the email address is not valid |
| 403 | Not an administrator; another hospital's doctor; deleting a user |
| 404 | No such account |
| 409 | An account with that email already exists |

Deactivating a doctor also rejects their existing tokens ("this account has been deactivated
by an administrator"): the server drops its cached sign-ins. It then emits `user.deactivated` on
the `access-changes` topic, and reactivation emits `user.reactivated`.

## 4. Database (v0.3)

`Server/db/schema.cql` adds, in place and safe to run again:

- `users`: `email`, `status` (null or `ACTIVE` / `DEACTIVATED`), `created_by`,
  `deactivated_at`, `deactivated_by`.
- `users_by_email (email PRIMARY KEY, user_id)`: the uniqueness check (R6) and first-sign-in link.

An existing trial database is upgraded by the `cassandra-init` step. It does not need to be
recreated.

## 5. Apps

**Settings → Administration** (Android `ui/admin/AdminScreen.kt`, iOS
`Features/AdminView.swift`) shows:

- **Add user** and **Add doctor** forms. The doctor form offers designation chips
  (Cardiologist, Urologist, Paediatrician) and accepts any other designation typed in.
- The hospital's doctor list, where each doctor has **Delete** (with a confirmation that
  explains deactivation) or **Reactivate**.

Users have no delete button. In demo mode (no server), a "Hospital A Admin" account in the fake
repository makes the screen usable.

## 6. Trial accounts

The seed job (`care-seed`) adds nine accounts for testing these screens and the page
operations. It creates any that are missing, so an existing trial database gets them without a
reset. They are listed in `Server/README.md` and in the appendix of `userapidocumentation.txt`:
3 patients, 2 independent doctors (Paediatrician, Cardiologist), 2 doctors at Test Hospital A
(Cardiologist, Urologist) and 2 labs. Sign in as **Hospital A Admin (HL-8A2D4)** to use
Administration.

## 7. Tests

**Unit tests** (`cargo test`, `policy.rs`), 26 of 26 pass:

| Test | Checks |
| --- | --- |
| A1 | Administrators create users and doctors, and nothing else |
| A2 | Non-administrators can't create accounts |
| A3 | A hospital administrator deletes a doctor of their own hospital |
| A4 | …but not a doctor of another hospital |
| A5 | Users (and other non-doctor accounts) can never be deleted |
| A6 | A hospital administrator acts only for their own hospital; a platform administrator must name one |
| — | Seed tests: Healoo IDs are unique and in the sign-in list; the new accounts cover the requested mix |

**Against the LAN trial server** (curl, 2026-09-27), all as expected:

| Check | Result |
| --- | --- |
| `/dev/users` lists all 19 trial accounts | ✓ |
| Dr. Kavya Menon: "Doctor · Cardiologist", Test Hospital A | ✓ |
| Admin lists doctors: Anitha Rao, Kavya Menon, Vikram Nair | ✓ |
| Create user | 201 |
| Same email again | 409 |
| Create doctor (Paediatrician), joins Test Hospital A | 201 |
| A doctor calls `/v1/admin/users` | 403 |
| Delete a user | 403 |
| Deactivate the new doctor | 204; hidden from search; their token gets 403 |
| Reactivate | 200 |
| Deactivate another hospital's doctor (Dr. Srinivas Rao) | 403 |

**Not yet tested:** the Administration screens on a device, iOS compilation, and linking an
Auth0 sign-in by email (needs `AUTH_MODE=auth0`).

## 8. Platform administrator dashboard (server v0.4)

A web dashboard for platform administrators (APP_ADMINISTRATOR), served by the API itself at
**`https://<API_DOMAIN>/admin/`** (LAN trial: `https://<lan-ip>:8443/admin/`). It opens in Edge
or Chrome on Windows; nothing is installed. Plain HTML, CSS and JavaScript built into the
`care-api` binary (`Server/care-api/src/dashboard/`), no libraries and no CDN.

| Page | Shows |
| --- | --- |
| Overview | Accounts, active users today, today's sign-ups, messages, bookings, reports and uploads; a 14-day chart of any counter against active users; accounts by role; the latest activity |
| Usage | For the last **day, week (7 days), month (30 days) and year (365 days)**: active users, active hospitals, messages, reports, files uploaded, bookings, sign-ups and every other counter. **Data stored**: database size per table, record counts, number and size of stored files |
| Accounts | Every account, searchable by name, Healoo ID or email, filtered by role and status (active, deactivated, never signed in) |
| Activity | One day's activity log (UTC), filtered by kind: records, sharing and access, accounts and hospitals |
| Hospitals | Every hospital with its details, doctor count and administrators; **add a hospital** (optionally with its first administrator); **appoint** an existing account as a hospital's administrator by Healoo ID; remove one |

### 8.1 Decisions

| # | Decision |
| --- | --- |
| R7 | The dashboard and its API are for **platform administrators only**. Hospital administrators keep using Settings → Administration in the app. |
| R8 | The activity log says **who did what, when, and whose record or account it was**. It never holds titles, message text, report contents, vital readings or reminder text. Vital alerts and reminders firing are left out. |
| R9 | Only platform administrators **create hospitals**. A hospital administrator edits only their own hospital (section 9). |
| R10 | A hospital administrator joins with an **invite code** (section 11, the usual way), is created with the hospital (name + email; linked on their first Auth0 sign-in with that verified email), or is **appointed**: the person signs up in the app, then a platform administrator enters their Healoo ID. Doctors, assistants, labs and hospital accounts can't be appointed (they keep one clinical role). |
| R11 | The first platform administrator is made on the server: `care-admin grant <Healoo ID>`. The dashboard can't create platform administrators. |
| R12 | A **booking** is a new appointment record; a **report** is a new report record. Appointment changes and files uploaded are counted separately. |
| R13 | A hospital is **active** in a period when the hospital account, one of its administrators or one of its doctors (by home hospital) did something. A user is active when they did anything that is logged. |

### 8.2 How it works

The API already emits events for everything that matters (design doc 6). A new consumer in
`care-worker`, **`activity-log`** (`src/activity.rs`), reads `items.events`, `chat.messages`,
`access.changes` and `access.audit` and writes three tables:

| Table | Holds | Kept |
| --- | --- | --- |
| `activity_by_day ((day), at)` | One line per event: category, type, actor, subject, a plain-words detail | 90 days |
| `activity_counts ((day), metric)` | Counters per day (`messages`, `bookings`, `reports`, `uploads`, …) | forever |
| `active_users_by_day ((day), user_id)` | Who did something that day | 400 days |

New events: `user.registered` (first sign-in), `user.created` (an administrator created an
account), `hospital.created`, `hospital.updated`, `admin.appointed`, `admin.removed`. The consumer
reads from the start of each topic, so events Kafka still holds are logged too; older activity
is not counted.

**Data stored.** Database size comes from Cassandra's `system_views.disk_usage` (whole MiB per
table; data still in memory is counted once Cassandra flushes it to disk). Record counts are
`COUNT(*)` of items, messages, attachments, appointments and vital readings, each given 10
seconds. File storage walks the bucket (S3 / SeaweedFS) or the files folder (LAN).

The account list and the usage page scan every account, and the usage page reads 365 days of
counters. That is fine for one hospital group; a very large deployment would add summary tables.

### 8.3 API (APP_ADMINISTRATOR only; anything else gets 403)

| Method | Path | Result |
| --- | --- | --- |
| GET | `/v1/admin/overview?days=14` | `accounts` (total, active, deactivated, never_signed_in, by_role), `today`, `trend` (per day: active_users + every counter), `recent` (10 lines), `metrics` |
| GET | `/v1/admin/usage` | `periods` (day/week/month/year: active_users, active_hospitals, totals), `users_total`, `hospitals_total`, `storage` (database_mib, tables, records, files) |
| GET | `/v1/admin/accounts?q=&role=&status=&offset=&limit=` | `data` (accounts, sorted by name), `total` |
| GET | `/v1/admin/activity?day=YYYY-MM-DD&category=` | `day`, `data` (newest first) |
| GET | `/v1/admin/hospitals` | `data`: profile, `doctors`, `administrators` |
| POST | `/v1/admin/hospitals` | 201 `{hospital, administrator}`. Body: section 9 fields + optional `admin_name`, `admin_email` |
| POST | `/v1/admin/hospitals/{id}/administrators` | `{"public_id": "HL-…"}` → the account is the hospital's administrator |
| DELETE | `/v1/admin/hospitals/{id}/administrators/{user_id}` | 204; the account keeps its other roles |

### 8.4 Signing in

- **Internet (Auth0):** an Auth0 **Single Page Application** ("Healoo Admin Dashboard") with
  Allowed Callback URLs and Allowed Logout URLs `https://<API_DOMAIN>/admin/` and Allowed Web
  Origins `https://<API_DOMAIN>`. Its client ID goes in `ADMIN_AUTH0_CLIENT_ID` in
  `deploy/internet/.env`. The page uses Authorization Code with PKCE and asks for the same API
  audience as the apps. The token is kept for the browser tab only (sessionStorage).
- **LAN trial (AUTH_MODE=dev):** the page lists the test accounts; **Healoo Platform Admin
  (HL-9A7D2)** is a seeded platform administrator.
- To make someone a platform administrator (they sign up in the app first):
  `docker compose run --rm care-api care-admin grant HL-XXXXX` (also `revoke`, `list`).

The page sends a strict Content-Security-Policy: scripts and styles from the server only, and
connections only to the server and the Auth0 tenant.

## 9. Hospital profile (server v0.4, Android and iOS apps)

A hospital administrator edits their hospital's **name, headline, address, phone and
registration number** in the app: **Settings → Administration → Hospital profile → Edit**.
Patients and doctors see the name, headline and address on the hospital's profile.

| Method | Path | Result |
| --- | --- | --- |
| GET | `/v1/admin/hospital` | The administrator's hospital profile |
| PUT | `/v1/admin/hospital` | Replaces the profile; empty fields are cleared |

```json
{
  "display_name": "Test Hospital A",
  "headline": "Multi-speciality hospital",
  "location": "12 MG Road, Bengaluru",
  "phone": "+91 80 4000 1234",
  "official_number": "KA-HOSP-2024-0117",
  "hospital_id": null
}
```

`display_name` is 1–80 characters, `location` up to 120, `headline` up to 80, `official_number`
up to 40, `phone` up to 30 (digits, spaces, `+ - ( )`). A hospital administrator always edits
their own hospital (`hospital_id` is ignored); a platform administrator names it. 403 for anyone
else or another hospital; 404 when the id is not a hospital. Saving emits `hospital.updated`.

The iOS app has the same screen (`Features/AdminView.swift`); it is written but not yet compiled.

### 9.1 Tests

`cargo test` adds: A7 (a hospital administrator edits only their own hospital), A8 (the dashboard
API is for platform administrators only), A9 (who can be appointed), and the activity-log tests
(records, sharing and accounts are logged; message text never reaches the log; only refused
access is logged; automatic and health events are left out).

## 10. Encrypted administrators file (server v0.4)

Who the administrators are can be kept in one encrypted file, **`Server/config/admins/admins.enc`**,
instead of being set account by account. It lists each administrator's **name, email and Healoo
ID** (user ID). The file is never committed (`.gitignore`) and never copied into an image
(`Server/.dockerignore`); the deploy folders mount it into the API container.

| # | Decision |
| --- | --- |
| R14 | The file is read with a **key phrase** (`ADMINS_KEY`, set in `deploy/*/.env`). The phrase can be changed at any time with `care-admin config rekey`. |
| R15 | `care-api` applies the file at **every start**. **Platform administrators are exactly the ones listed**: listed accounts get the role, any other account loses it. |
| R16 | **Hospital administrators** listed in the file are made administrators of the named hospital. Administrators appointed in the dashboard and not listed are left alone. |
| R17 | An entry is found by its Healoo ID (or UUID), then by email. If neither exists and an email is given, the account is **created** with that email; the person's first Auth0 sign-in with the same verified email is linked to it. An existing account without an email gets the one in the file. |
| R18 | A wrong key phrase, an edited file or invalid contents change **nothing**; the API logs an error and starts normally. A missing file is fine. |
| R19 | Doctors, assistants, labs and hospital accounts can't be made administrators through the file either (same rule as section 8, R10). |

### 10.1 Format and encryption

Plain contents (JSON, only ever seen by `care-admin config show --json` or `import`):

```json
{
  "platform_admins": [
    { "name": "Healoo Platform Admin", "email": null, "user_id": "HL-9A7D2" },
    { "name": "Asha Verma", "email": "asha@example.com", "user_id": null }
  ],
  "hospital_admins": [
    { "name": "Hospital A Admin", "email": null, "user_id": "HL-8A2D4", "hospital": "HL-1H0A1" }
  ]
}
```

Every entry needs a name and at least an email or a `user_id`.

On disk: a first line `HEALOO-ADMINS-1`, then base64 of a 16-byte salt, a 12-byte nonce and the
ciphertext. The key is **Argon2id**(key phrase, salt) (32 bytes, default parameters); the cipher
is **ChaCha20-Poly1305**, which also detects any change to the file. Each save uses a new salt and
nonce.

### 10.2 Commands

Run from `Server/deploy/internet` (or `deploy/lan`); `ADMINS_KEY` comes from `.env`:

| Command | Does |
| --- | --- |
| `docker compose run --rm care-api care-admin config init` | New file with the seeded test administrators |
| `… care-admin config show` (`--json`) | Lists the administrators |
| `… care-admin config add-platform "<name>" <email or -> <Healoo ID or ->` | Adds a platform administrator |
| `… care-admin config add-hospital "<name>" <email or -> <Healoo ID or -> <hospital Healoo ID>` | Adds a hospital administrator |
| `… care-admin config remove <name, email or Healoo ID>` | Removes an entry |
| `… care-admin config import <file.json>` | Replaces the list with a plain JSON file (delete that file afterwards) |
| `docker compose run --rm -e ADMINS_NEW_KEY=<new phrase> care-api care-admin config rekey` | Re-encrypts with a new key phrase; then put it in `ADMINS_KEY` |
| `… care-admin config apply` | Applies the file now, without restarting `care-api` |

`care-admin grant` / `revoke` still change a single account, but the file wins at the next start.

The ones used most, ready to copy (from `Server/deploy/internet`):

List the administrators in the file:

```bash
docker compose run --rm care-api care-admin config show
```

Add a platform administrator (name, email, then a Healoo ID or `-` for none):

```bash
docker compose run --rm care-api care-admin config add-platform "Your Name" you@example.com -
```

Remove someone (by name, email or Healoo ID):

```bash
docker compose run --rm care-api care-admin config remove you@example.com
```

Apply the file now, without restarting care-api:

```bash
docker compose run --rm care-api care-admin config apply
```

Change the key phrase (then put the new phrase in `ADMINS_KEY` in `.env`):

```bash
docker compose run --rm -e ADMINS_NEW_KEY=newphrase care-api care-admin config rekey
```

### 10.3 Tests

`cargo test`: the file decrypts only with the right phrase and its contents are not readable;
each encryption differs and a changed byte is detected; entries need a name and an email or ID.

## 11. Invite codes for hospital administrators (server v0.4, Android and iOS apps)

The usual way to add a hospital administrator. It needs no email matching and no Auth0
configuration beyond normal sign-in.

1. A platform administrator opens the dashboard → **Hospitals** → the hospital → **Invite codes**,
   optionally notes who it is for, and selects **Create invite code**. The code (e.g.
   `K7QM-4XTR-9HCW`) is shown **once**, with a Copy button.
2. They send the code to the person by any means (phone, chat, email).
3. The person signs in to the Healoo app with Auth0 (any email or Google), opens **Settings →
   Hospital administration → I have an invite code** and enters it.
4. They are now that hospital's administrator: Settings → Administration appears, including the
   hospital profile (section 9). The dashboard shows the code as **Used** by them.

| # | Decision |
| --- | --- |
| R20 | Codes are **single use** and **expire after 7 days**. A platform administrator can **revoke** a code that hasn't been used. |
| R21 | The server stores only a **SHA-256 hash** of each code; the code itself exists only in the create response. |
| R22 | Codes are 12 characters from 31 easily read letters and digits (no 0/O, 1/I/L), shown in three groups. Case, spaces and dashes don't matter when typing. |
| R23 | Every wrong, used, expired or revoked code gets the **same answer**. After **5 wrong codes in 15 minutes** an account must wait (429). |
| R24 | Only accounts that may be administrators can use a code (section 8, R10): not doctors, assistants, labs or hospitals. Someone who already administers another hospital is refused (409). |
| R25 | Two people entering the same code at once can't both succeed: the code is marked used with a Cassandra lightweight transaction (`IF used_by = null`). |

### 11.1 API

| Method | Path | Who | Result |
| --- | --- | --- | --- |
| POST | `/v1/admin/hospitals/{id}/invites` | Platform administrator | 201 `{invite_id, code, expires_at, status, note, hospital_name, valid_days}`. Body `{"note": "…"}` (optional) |
| GET | `/v1/admin/hospitals/{id}/invites` | Platform administrator | `data`: invites, newest first, with `status` (PENDING, USED, REVOKED, EXPIRED) and `used_by`; never the codes |
| DELETE | `/v1/admin/hospitals/{id}/invites/{invite_id}` | Platform administrator | 204; 409 if already used |
| POST | `/v1/invites/redeem` | Anyone signed in | `{"code": "K7QM-4XTR-9HCW"}` → `{hospital_id, hospital_name}`; 400 invalid code, 403 clinical account, 409 already administers another hospital, 429 too many wrong codes |

Tables: `admin_invites` (by id), `admin_invites_by_code` (hash → id) and
`admin_invites_by_hospital`. Events: `invite.created`, `invite.redeemed` (activity log:
"created an administrator invite code for …", "became an administrator of … with an invite
code"; counters `invites_created` and `admins_appointed`).

In demo mode (no server) the code `ABCD-EFGH-JKMN` makes the signed-in person Test Hospital A's
administrator.

### 11.2 Tests

`cargo test` (`store/invites.rs`): codes are readable, unique and forgiving to type; malformed
codes are rejected before any lookup; status follows use, revocation and expiry.

## 12. How many administrators (server v0.4)

Two settings in **`Server/config/server.json`** limit the number of administrators:

```json
"administrators": {
  "max_platform_admins": 3,
  "max_admins_per_hospital": 2
}
```

| Setting | Default | Meaning |
| --- | --- | --- |
| `max_platform_admins` | 3 | Platform administrators (dashboard users) in total |
| `max_admins_per_hospital` | 2 | Hospital administrators per hospital |

Each must be 1–100; a file outside that range stops the server at start-up with the reason. To
change them, edit `server.json` and restart `care-api` and `care-worker` (both deploy folders
mount the file; no rebuild).

| Where an administrator is added | When the limit is reached |
| --- | --- |
| Administrators file (section 10) | A file listing more platform administrators than allowed is **not applied at all** (logged as an error; `care-admin config add-platform` refuses to add one more). A hospital administrator entry beyond the limit is skipped and reported. |
| `care-admin grant` | Refused |
| Dashboard → Appoint administrator | 409, "already has 2 administrators" |
| Dashboard → Create invite code | 409 when administrators **plus unused codes** would exceed the limit, so every code handed out can still be used |
| App → I have an invite code | 409 if the hospital filled up in the meantime; the code stays unused |
| New hospital with its first administrator | Always allowed (one administrator) |

Existing administrators are never removed by lowering a limit; it only stops new ones. The
dashboard shows each hospital's count against the limit ("Administrators · 1 of 2").

Tests: `settings.rs` S1 and S4 (defaults and the 1–100 check) and `admins_file.rs` (a file with
more platform administrators than the limit is refused).
