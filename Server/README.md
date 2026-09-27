# Healoo backend (`care-api`) — v0.2 (DataItem v2)

Rust backend for design doc sections **4** (REST + WebSocket on Tokio), **5** (Auth0),
**6** (Kafka) and **7** (Cassandra + files). One codebase runs in two network profiles:

- **LAN trial** — phones and emulators on the same Wi-Fi talk straight to your computer.
  Works fully offline with dev tokens.
- **Internet** — deployed behind Caddy or Nginx with public certificates and Auth0.

> ⚠️ **Not compiled yet.** This was written without a Rust toolchain available. It has been
> checked structurally, but expect a handful of compile fixes on the first `cargo build`
> (most likely around exact `scylla 0.13` driver type paths). Run `cargo build` and
> `cargo test` first; the policy unit tests cover acceptance tests 2–7.

---

## 1. How LAN vs internet is abstracted

Everything that differs between the two profiles sits behind one trait each. Handlers only
see the traits, and `NETWORK_MODE` picks the implementations and safe defaults.

| Concern | Trait (file) | LAN trial | Internet |
| --- | --- | --- | --- |
| Network exposure | `Exposure` (`src/net/`) | `LanExposure`: listens on all interfaces, HTTPS with its own local CA (or behind local Caddy), mDNS `_healoo._tcp`, permissive CORS | `InternetExposure`: private port behind Caddy/Nginx, `X-Forwarded-For` trusted only from proxy ranges, strict CORS |
| Identity | `TokenVerifier` (`src/auth/`) | `DevVerifier`: locally signed tokens, no internet needed (Auth0 optional) | `Auth0Verifier`: RS256 + JWKS cache (dev tokens refused) |
| Files | `ObjectStore` (`src/files/`) | `DiskStore`: files on disk, served by the API at `/files` with signed, expiring URLs | `S3Store`: S3/MinIO presigned URLs on a files subdomain |
| Events | `EventBus` (`src/events/`) | `KafkaBus` (or `MemoryBus` for a Kafka-less quick run) | `KafkaBus` |

| Setting | `NETWORK_MODE=lan` default | `NETWORK_MODE=internet` default |
| --- | --- | --- |
| Listen | `0.0.0.0:8443` (HTTPS) | `0.0.0.0:8080` (HTTP, private network) |
| Public URL | `https://<LAN IP>:8443/` (auto-detected or `PUBLIC_BASE_URL`) | `PUBLIC_BASE_URL`, **must be https** |
| Auth | `AUTH_MODE=dev` | `AUTH_MODE=auth0` (dev is refused at startup) |
| Files | `FILE_STORE=disk` | `FILE_STORE=s3` |
| Debug routes (`/v1/check/access`, `/dev/*`) | on | off |
| Logs | readable text | JSON lines |

Startup refuses unsafe combinations (dev tokens on the internet, non-HTTPS public URL,
default file-signing secret with public disk storage).

## 2. What's implemented

**Section 4 — Communication.** axum on Tokio; WebSocket via axum's tokio-tungstenite support.
REST routes from doc 4.3 with the DataItem v2 item model (`Documentation/DataItem_Design.md`):
items created from one primary part, and inside each item messages, attachments, recurring
appointments (per-visit cancel/move/attended/missed), alerts, close with feedback and rating,
reopen; plus `/v1/conversations` (Messages tab, replaces `/v1/threads`), `/v1/appointments`
(calendar), `PATCH /v1/me`, `/v1/me/notification-prefs`, `GET /v1/grants?owner=me`,
`DELETE /v1/devices/{token}` and `/v1/config`. Full reference: `Documentation/userapidocumentation.txt`. Error shape `{"error":{code,message,request_id}}`. Limits from doc 4.5: 20 MB
bodies, 20 attachments, 10 MB images / 25 MB PDFs, 64 KB WS frames, 20 req/s per user,
5 sockets per user. WebSocket at `/v1/ws` (apps) and `/ws` (doc): Bearer header, `?token=`, or
first-frame auth; 25 s ping, 60 s silence drop, close 4001 on token expiry; idempotent
`message.send {item_id, body, client_msg_id}`; typing relay to the item's participants.

**Section 5 — Auth0.** JWKS cached 10 min, refreshed on unknown `kid`; checks `iss`, `aud`,
`exp`; reads the namespaced `roles` / `uid` claims from the post-login Action; provisions a
PATIENT on first login. Fine-grained access stays in the policy engine (`src/policy.rs`),
which implements rules 1–8 of doc 2.3 and logs every decision to `access.audit`.

**Section 6 — Kafka.** Topics and DLQs from doc 6.1 (`kafka/create-topics.sh`), doc 6.2
envelope, idempotent producer (`acks=all`), at-least-once consumers that store offsets only
after the handler succeeds, 3 retries then `<topic>.dlq`, Cassandra outbox + relay so no event
is lost between the write and the publish, `processed_events` dedupe. Consumers:
`ws-fanout` (in care-api, pushes to sockets and emits `notify.requests` for offline users),
`audit-writer`, `media` and the alert scheduler (in care-worker; fires due alerts every 20 s
as push + WebSocket `alert` frames).

**Section 7 — Cassandra + files.** `db/schema.cql` (doc 7.2 extended; changes marked `v0.2`).
Query-shaped tables, logged batches for grant/revoke/status moves (doc 7.3), SAI name search.
Files never touch Cassandra: presigned uploads, 10-minute view URLs issued only after the
policy check, and a media worker that makes image thumbnails, counts PDF pages and records
SHA-256.

## 3. Run the LAN trial (Docker)

Needs Docker Desktop (or Docker Engine) and ~6 GB free RAM.

```bash
cd deploy/lan
cp .env.lan.example .env        # set HOST_LAN_IP to this computer's Wi-Fi address
docker compose up -d --build    # first build of the Rust image takes several minutes
docker compose logs -f care-api # wait for "listening (HTTPS, built-in)"
```

The seed job loads the same people and records as the apps' demo mode, plus a hospital
admin (`HL-8A2D4`) and an assistant (`HL-8S5T7`):

| Healoo ID | Name | Role |
| --- | --- | --- |
| HL-2M9P4 | Lakshmi K. | Patient |
| HL-7R2C9 | Dr. Anitha Rao | Doctor at Test Hospital A |
| HL-1H0A1 | Test Hospital A | Hospital |
| HL-6L3D2 | City Diagnostics | Lab |
| HL-3K8M1 | Dr. Srinivas Rao | Independent doctor |
| HL-4K7Q2 | Priya Rao | Patient |
| HL-8A2D4 | Hospital A Admin | Hospital administrator |
| HL-8S5T7 | Meena S. | Assistant to Dr. Rao |
| HL-3R7V2 | Ravi Kumar | Patient (contacts: Dr. Menon, Hospital A, Sunrise) |
| HL-6A4S9 | Anjali Sharma | Patient (contacts: Dr. Mehta, Green Valley) |
| HL-9S2Y5 | Suresh Iyer | Patient (contacts: Dr. Khan, Dr. Nair, Sunrise) |
| HL-4M8D3 | Dr. Arjun Mehta | Independent doctor · Paediatrician |
| HL-7F5K2 | Dr. Farah Khan | Independent doctor · Cardiologist |
| HL-2D5M8 | Dr. Kavya Menon | Doctor at Test Hospital A · Cardiologist |
| HL-8N3V6 | Dr. Vikram Nair | Doctor at Test Hospital A · Urologist |
| HL-5P9L4 | Sunrise Pathology | Lab |
| HL-6G2L7 | Green Valley Labs | Lab |

The last nine are added to an existing trial database too: the seed job creates any that are
missing on the next `docker compose up -d --build`, without touching other data.

**Trust the local CA once per device.** On first start the API writes
`healoo-local-ca.crt` into the `app_data` volume:

```bash
docker compose cp care-api:/data/tls/healoo-local-ca.crt .
```

Install it on the Android emulator/phone (Settings → Security → Encryption & credentials →
Install a certificate → CA certificate; the debug build already trusts user CAs) and on the iOS
Simulator (drag the file onto it, then Settings → General → About → Certificate Trust Settings).
The certificate covers your LAN IP, `localhost` and `10.0.2.2`.

Kafka UI: http://localhost:8081. Cassandra and Kafka listen on 127.0.0.1 only; port 8443 is
the only one reachable from the LAN.

### Without Docker for the API (faster edit–run loop)

Start only the infrastructure, then run the binaries from the `care-api` folder:

```bash
cd deploy/lan && docker compose up -d cassandra cassandra-init kafka kafka-init kafka-ui
cd ../../care-api
export NETWORK_MODE=lan CASSANDRA_NODES=127.0.0.1:9042 KAFKA_BROKERS=localhost:29092
cargo run --bin care-seed
cargo run --bin care-api      # also advertises _healoo._tcp over mDNS when run natively
cargo run --bin care-worker   # in a second terminal
```

With `EVENT_BUS=memory` you can even skip Kafka; the worker then runs inside care-api.

## 4. Try it with curl

```bash
BASE=https://192.168.1.20:8443     # your HOST_LAN_IP
TOKEN=$(curl -sk -X POST $BASE/dev/token -H 'content-type: application/json' \
         -d '{"public_id":"HL-2M9P4"}' | sed -E 's/.*"access_token":"([^"]+)".*/\1/')
curl -sk $BASE/v1/me        -H "authorization: Bearer $TOKEN"
curl -sk $BASE/v1/items     -H "authorization: Bearer $TOKEN"
curl -sk "$BASE/v1/search?q=rao" -H "authorization: Bearer $TOKEN"
# WebSocket (websocat): expect {"type":"ping"} every 25 s
websocat -k "wss://192.168.1.20:8443/v1/ws?token=$TOKEN"
```

Policy checks (acceptance test 11): `GET /v1/check/access?item=<id>&user=<id>` returns the
decision and reason. `GET /dev/users` lists the seeded IDs. `devtoken <uuid> [roles]` prints a
token from the command line.

## 5. Connect the apps (WUI 0.3)

Android `android/gradle.properties`:

```
healoo.useFakeData=false
healoo.apiBaseUrl=https://192.168.1.20:8443/     # emulator can also use https://10.0.2.2:8443/
```

iOS `ios/project.yml`: `HealooUseFakeData: NO` and `HealooAPIBaseURL: https://192.168.1.20:8443/`,
then `xcodegen generate`.

**Sign-in:** WUI 0.3 debug builds show **Developer sign-in** when `healoo.devSignIn` /
`HealooDevSignIn` is on: pick a seeded account or type a Healoo ID; the app calls
`POST /dev/token` and renews the token itself. With `AUTH_MODE=auth0` the Auth0 button works
as before (the computer needs internet only for the login itself).

Compatibility notes:
- Realtime frames match the apps: `message.new {item_id, message}`, `item.updated {item_id,
  change}` with `change` in `created | message | attachment | appointment | alert | closed |
  reopened | shared | updated`, `alert`, `typing`, `ping`.
- `sent_at`, `fires_at` and `last_message_at` are ISO-8601 with offset; the apps format them.
- **Upgrading from v0.1:** the schema changed (DataItem v2). Recreate the trial database and
  seed again: `docker compose down -v`, then `docker compose up -d --build` (the `care-seed` job
  reloads the test data).
- Thumbnails and PDF page counts appear a moment after upload (the media worker fills them
  in, then sends `item.updated`).

## 6. Deploy on the internet

1. A Linux server with Docker, and DNS records for two names, e.g. `api.example.com` and
   `files.example.com` (files need their own host because presigned URLs are signed for the
   exact host and path).
2. Auth0 tenant set up as in doc 5.1–5.3 (API audience, roles, post-login Action).
3. Then:

```bash
cd deploy/internet
cp .env.internet.example .env     # domains, ACME email, Auth0, MinIO credentials
docker compose up -d --build
```

Caddy gets Let's Encrypt certificates automatically; `proxy/nginx.conf` is the equivalent if
you prefer Nginx. Only ports 80/443 are published. Point the apps at `https://api.example.com/`.
To load test data there: `docker compose run --rm care-api care-seed` (test data only).

## 7. Configuration reference

| Variable | Meaning |
| --- | --- |
| `NETWORK_MODE` | `lan` or `internet` — picks all defaults below |
| `PUBLIC_BASE_URL` | URL phones use (LAN: auto from LAN IP; internet: required, https) |
| `BIND` | listen address |
| `LAN_TLS` | `builtin` (own CA), `proxy` (Caddy on this machine, `proxy/Caddyfile.lan`), `off` |
| `TLS_DIR` | where the local CA and server certificate live |
| `MDNS` | advertise `_healoo._tcp` on the LAN |
| `TRUSTED_PROXIES` | CIDRs allowed to set `X-Forwarded-For` |
| `CORS_ORIGINS` | comma-separated web origins (internet) |
| `AUTH_MODE` | `auth0` or `dev` |
| `AUTH0_DOMAIN`, `AUTH0_AUDIENCE`, `CLAIMS_NAMESPACE` | Auth0 API settings (doc 5) |
| `DEV_JWT_SECRET` | signing secret for dev tokens |
| `CASSANDRA_NODES`, `CASSANDRA_KEYSPACE` | Cassandra contact points, keyspace `careconnect` |
| `EVENT_BUS`, `KAFKA_BROKERS` | `kafka` or `memory`; broker list |
| `FILE_STORE`, `FILES_DIR`, `FILES_SECRET` | `disk` or `s3`; disk folder; URL signing key |
| `S3_BUCKET`, `S3_ENDPOINT`, `S3_PUBLIC_ENDPOINT`, `S3_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | S3/MinIO |
| `INSTANCE_ID` | names this instance's ws-fanout consumer group |
| `DEBUG_ROUTES` | expose `/v1/check/access` |
| `RUST_LOG` | log filter |

## 8. Layout

```
care-api/src/
  config.rs          profile selection + validation
  net/               Exposure trait; lan.rs, internet.rs, tls.rs (local CA)
  auth/              TokenVerifier trait; Auth0 + dev verifiers
  policy.rs          access rules (doc 2.3) + unit tests
  model.rs           enums, Cassandra UDTs, JSON shapes
  store/             Cassandra repositories (users, items, social, misc)
  events/            envelope, EventBus trait, Kafka + memory buses, outbox publisher
  files/             ObjectStore trait; disk + S3 stores
  api/               REST handlers, principal extractor, error shape, router
  ws/                WebSocket gateway + Kafka fan-out
  worker.rs          audit-writer + media processor
  seed.rs            trial data
  bin/               care-api, care-worker, care-seed, devtoken
db/schema.cql        Cassandra schema
kafka/create-topics.sh
deploy/lan/, deploy/internet/   docker-compose profiles + example .env files
proxy/               Caddyfile.internet, Caddyfile.lan, nginx.conf
Dockerfile           targets: api, worker
```

## 9. Not included yet / known limits

- **Section 8 notifier** (FCM sender) — `notify.requests` events are produced; the
  `care-notifier` binary that sends them is the next step.
- Alert scheduler and search indexer consumers (doc 6.4).
- List endpoints merge several partitions and return up to 100 rows without a `page_state`
  cursor yet.
- Presence is per API instance; run one care-api instance until presence moves to a shared
  store.
- PDF first-page thumbnails need a PDF renderer (e.g. pdfium); images get thumbnails now.
- Presigned PUTs don't enforce size at the storage layer; sizes are checked at presign and
  item creation.
- Trial and test data only. Before any real patient data, see doc 10.6 (DPDP Act 2023, ABDM
  guidelines, encryption at rest, backups, multi-node clusters, security audit).
