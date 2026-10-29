# Third-party configuration — cheat sheet

Every certificate, key, outside service, configuration file and setting Healoo depends on, on
one page. Details are in the documents linked in each section.

**Never commit:** `.env` files, `*.key`, `*.p12`, `*.p8`, `*.jks`, `admins.enc`,
`google-services.json`, `GoogleService-Info.plist`, `.mcp.json`. `.gitignore` already
excludes them.

---

## 1. Certificates and keys

### 1.1 HTTPS certificates (how phones and browsers trust the server)

Which ones are in use depends on how the server runs. **Each setup uses one certificate
authority (CA) that devices must trust, or none on the internet.**

| # | Certificate | Setup | Type | Valid | Created by | Where it lives | Who must trust it |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | **Healoo Local Trial CA** (`healoo-local-ca.crt`) | LAN trial (`deploy/lan`) | Root CA, ECDSA P-256, SHA-256 | Until year 4096 | `care-api` on first start (`src/net/tls.rs`) | Container `/data/tls` (volume `app_data`); copy at `Server/deploy/lan/healoo-local-ca.crt` | Android debug builds include it automatically; iPhones, other devices and browsers must install it |
| 2 | LAN server certificate (`server-<ip>.crt`) | LAN trial | Leaf, signed by #1. Names: LAN IP, `localhost`, `healoo-trial.local`, `10.0.2.2` | Re-issued when the LAN IP changes | `care-api` | `/data/tls` | Nothing to do (trusted through #1) |
| 3 | **Let's Encrypt** certificates, one for `API_DOMAIN` and one for `FILES_DOMAIN` | Internet (`deploy/internet`, real domains) | Public, issued by Let's Encrypt | 90 days, renewed automatically | Caddy (`ACME_EMAIL` in `.env`) | Volume `caddy_data` | Nobody: public CAs are trusted everywhere |
| 4 | **Caddy Local Authority** root (`healoo-internet-local-ca.crt`) | Internet profile run locally with an IP address (`Caddyfile.local`) | Root CA, ECDSA P-256 | 10 years | Caddy (`local_certs`) | `caddy_data` → `/data/caddy/pki/authorities/local/root.crt`; copy in `WUI/android/certs/` (not committed) | Android debug builds (via `WUI/android/certs/`); browsers and iPhones install it |
| 5 | Caddy intermediate + server certificate for the IP | Same as #4 | ECDSA, signed by #4 | 7 days / 12 hours, renewed automatically | Caddy | `caddy_data` | Nothing to do |

- **Deleting the volume** (`app_data` or `caddy_data`, e.g. `docker compose down -v`) creates a
  **new** CA. Copy the new root again and rebuild the debug app
  (`WUI/android/certs/README.txt`).
- **Release Android builds and the App Store** trust only public CAs (#3). The local CAs (#1, #4)
  are for testing only.
- **Installing a CA:** on Windows, open the `.crt` → Install Certificate → Current User →
  *Trusted Root Certification Authorities*. On an iPhone, open the `.crt`, install the profile
  (Settings → General → VPN & Device Management), then enable it under Settings → General →
  About → *Certificate Trust Settings*. On Android, debug builds need nothing.

### 1.2 Signing keys and secrets

| Key / secret | Used for | Algorithm | Set in | Status |
| --- | --- | --- | --- | --- |
| **Auth0 signing keys** | Tokens the apps and dashboard send to the server | RS256 | Managed by Auth0; the server reads `https://<AUTH0_DOMAIN>/.well-known/jwks.json` (needs internet access) | Automatic |
| `DEV_JWT_SECRET` | Trial sign-in tokens (`/dev/token`, LAN with `AUTH_MODE=dev` only) | HS256 | `deploy/lan/.env` | Change the default |
| `FILES_SECRET` | Signed `/files/…` links of the disk store (LAN) | HMAC-SHA256 | `deploy/lan/.env` | Change the default |
| `S3_ACCESS_KEY`, `S3_SECRET_KEY` | SeaweedFS S3 gateway; presigned upload and download links | AWS Signature V4 | `deploy/internet/.env` (letters and digits only) | Required |
| `ADMINS_KEY` | Encrypted administrators file `config/admins/admins.enc` | Argon2id → ChaCha20-Poly1305 | `deploy/*/.env`; change with `care-admin config rekey` | Required if the file is used |
| Invite codes | Making hospital administrators | Only a SHA-256 of each code is stored | Created in the dashboard; single use, 7 days | — |
| Android **debug** keystore | Debug APKs | RSA | `~/.android/debug.keystore`, made by the Android SDK | Automatic |
| Android **release** keystore (`.jks`) | Release APKs | RSA | `healoo.signing.storeFile`, `…storePassword`, `…keyAlias`, `…keyPassword` in the Gradle user home `gradle.properties` | Create once; never lose it |
| Apple certificate (`.p12`) + provisioning profile | Signed iPhone builds (`build-device`) | — | GitHub secrets `IOS_CERTIFICATE_P12_BASE64`, `IOS_CERTIFICATE_PASSWORD`, `IOS_PROVISIONING_PROFILE_BASE64` | Needs the Apple Developer Program (US$99/year); not set up |
| App Store Connect API key (`.p8`) | TestFlight upload | ES256 | GitHub secrets `APP_STORE_CONNECT_KEY_ID`, `…_ISSUER_ID`, `…_KEY_P8_BASE64` | Optional |
| Free Apple ID | Sideloading `Healoo-unsigned.ipa` with Sideloadly or AltStore | Apple signs on install | — | 7-day installs |
| Firebase / APNs | Push notifications | — | `WUI/android/app/google-services.json`, `WUI/ios/Healoo/GoogleService-Info.plist` (or GitHub secret `GOOGLE_SERVICE_INFO_PLIST_BASE64`); APNs key uploaded in the Firebase console | Optional; without them the apps build with push off |

---

## 2. Outside services

| Service | What for | Configuration |
| --- | --- | --- |
| **Auth0** | Sign-in for the apps and the dashboard | Section 3 |
| **Let's Encrypt** | Public HTTPS certificates (internet) | Automatic through Caddy; `ACME_EMAIL`; ports 80 and 443 reachable from the internet |
| **Firebase Cloud Messaging** (+ APNs for iPhone) | Push notifications | Files in section 1.2; Firebase BoM 33.3.0 (Android), firebase-ios-sdk (iOS) |
| **GitHub Actions** | iPhone builds on `macos-15` with Xcode 16.4 (`.github/workflows/build-macos.yml`) | Variables and secrets in section 6 |
| **Docker Hub images** | Server dependencies | Section 7 |

---

## 3. Auth0

| Item | Value or rule |
| --- | --- |
| Tenant (development) | `dev-elevatesystems-healoo.us.auth0.com` (`AUTH0_DOMAIN`) |
| **API** | Identifier `https://api.healoo.local` (`AUTH0_AUDIENCE`, the apps' audience), signing RS256. Application Access Policy → **User-delegated access: all apps allowed** (otherwise every app must be authorised individually) |
| Claims namespace | `https://careconnect.local/` (`CLAIMS_NAMESPACE`): the server reads `…roles`, `…uid`, `…email`, `…email_verified` if a post-login Action adds them. Without an Action a first sign-in becomes a new patient account; linking by email needs the email claims |

**Applications** (client IDs are public values; no client secrets are used):

| Application | Type | Client ID (development) | Callback and logout URL | Web origin |
| --- | --- | --- | --- | --- |
| Healoo Android | Native | `VRngPGM6mcg4A8OGec6wXxkEwEskyVUh` | `healoo://<AUTH0_DOMAIN>/android/com.healoo.app/callback` | — |
| Healoo iOS | Native | `wnvXj9oul0m3wvtkcTPJ5Tt7UpMwMlOY` | `com.healoo.app://<AUTH0_DOMAIN>/ios/com.healoo.app/callback` | — |
| Healoo Admin Dashboard | Single Page Application | `ct2tV5PhfTQS1mNj2VpMRSNVUTudRqpE` | `https://<API_DOMAIN>/admin/` | `https://<API_DOMAIN>` |

All three: Authorization Code + PKCE, token endpoint authentication *None*, RS256. The custom
schemes (`healoo://`, `com.healoo.app://`) are fixed by the apps: Android `auth0Scheme` in
`app/build.gradle.kts`, iOS the bundle ID `com.healoo.app` (`project.yml`). A different bundle ID
needs a different callback URL.

---

## 4. Configuration files

| File | Read by | What it holds | Committed | To change |
| --- | --- | --- | --- | --- |
| `Server/config/server.json` | care-api, care-worker | Upload sizes, message length, rate limit, appointments, live connections, **administrator limits** (3 platform, 2 per hospital) | Yes | Edit and restart (both deploy folders mount it) — `Configuration.md` |
| `Server/config/vitals.json` | care-api | Vitals, ranges, colours, Home grid | Yes | Edit and restart (LAN mounts it; internet uses the built-in copy) — `Vitals_Design.md` |
| `Server/config/admins/admins.enc` | care-api at start, `care-admin` | Platform and hospital administrators: name, email, Healoo ID (encrypted) | **No** | `care-admin config …` — `Administration_Design.md` §10 |
| `Server/db/schema.cql` | `cassandra-init` | Database tables | Yes | Re-run the stack; changes are additive |
| `Server/deploy/lan/.env` | Docker Compose | `HOST_LAN_IP`, `AUTH_MODE`, `DEV_JWT_SECRET`, `FILES_SECRET`, Auth0, `ADMINS_KEY` | **No** (example: `.env.lan.example`) | Edit, `docker compose up -d` |
| `Server/deploy/internet/.env` | Docker Compose | Domains, `ACME_EMAIL`, Auth0, `ADMIN_AUTH0_CLIENT_ID`, storage keys, `ADMINS_KEY` | **No** (example: `.env.internet.example`) | Edit, `docker compose up -d` |
| `Server/proxy/Caddyfile.internet` | Caddy | Two sites (API with `/v1`, `/ws`, `/healthz`, `/admin`; files), Let's Encrypt | Yes | Edit, restart Caddy |
| `Server/proxy/nginx.conf` | Nginx (alternative to Caddy) | Same routes | Yes | — |
| `deploy/internet/Caddyfile.local`, `docker-compose.override.yml` | Caddy / Compose | Local test only: Caddy's own CA for an IP address, port 8444, proxy settings | **No** (`.git/info/exclude`) | Local only |
| `WUI/app-config.json` | Both apps (built in) | Layout, list sizes, offline cache, designations | Yes | Edit, rebuild apps — `Configuration.md` |
| `WUI/page-operations.json` | Both apps (built in) | Bottom-bar operations per page | Yes | Edit, rebuild — `PageOperations_Design.md` |
| `WUI/android/gradle.properties` | Android build | Defaults: `healoo.useFakeData`, `healoo.apiBaseUrl`, `healoo.devSignIn`, `healoo.auth0.*` | Yes (no secrets) | Override per machine in the Gradle user home `gradle.properties` (`GRADLE_USER_HOME`, e.g. `D:\Installers\.gradle`) |
| `WUI/android/certs/*.crt` | Android **debug** builds | CAs to trust (section 1.1) | **No** | Copy, rebuild |
| `WUI/ios/project.yml` | XcodeGen → Xcode | Bundle ID, Info.plist keys `HealooUseFakeData`, `HealooAPIBaseURL`, `HealooAuth0Domain`, `HealooAuth0ClientId`, `HealooAuth0Audience` | Yes | Edit and `xcodegen generate`, or GitHub variables (section 6) |
| `.mcp.json` | Claude Code | Auth0 MCP server for the developer's own use | **No** | — |

---

## 5. Server environment variables

| Variable | Default | Meaning |
| --- | --- | --- |
| `NETWORK_MODE` | `lan` | `lan` or `internet` |
| `AUTH_MODE` | `dev` on LAN, `auth0` on internet | Sign-in method |
| `AUTH0_DOMAIN`, `AUTH0_AUDIENCE`, `CLAIMS_NAMESPACE` | —, `https://api.careconnect.local`, `https://careconnect.local/` | Auth0 (section 3) |
| `ADMIN_AUTH0_CLIENT_ID` | empty | Dashboard sign-in (section 3) |
| `DEV_JWT_SECRET`, `FILES_SECRET` | built-in defaults | Section 1.2 (change them) |
| `ADMINS_FILE`, `ADMINS_KEY`, `ADMINS_NEW_KEY` | `config/admins/admins.enc`, —, — | Administrators file and re-keying |
| `SERVER_CONFIG`, `VITALS_CONFIG` | built-in copies | Section 4 |
| `LAN_TLS`, `TLS_DIR`, `BIND` | `builtin`, `./data/tls`, `0.0.0.0:8443` (LAN) / `0.0.0.0:8080` | HTTPS on the LAN (`builtin`, `proxy` or `off`) |
| `PUBLIC_BASE_URL` | — | The address the apps use (signed links are built with it) |
| `CASSANDRA_NODES`, `CASSANDRA_KEYSPACE` | —, `careconnect` | Database |
| `KAFKA_BROKERS`, `EVENT_BUS` | `127.0.0.1:9092`, `kafka` | Events (`memory` runs consumers inside care-api, LAN only) |
| `FILE_STORE`, `FILES_DIR` | `disk` on LAN, `s3` on internet; `./data/files` | Files |
| `S3_ENDPOINT`, `S3_PUBLIC_ENDPOINT`, `S3_BUCKET`, `S3_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | —, —, `careconnect`, `us-east-1` | S3-compatible storage |
| `INSTANCE_ID`, `RUST_LOG` | random, `info` | Logging |
| `HTTPS_PROXY`, `HTTP_PROXY`, `NO_PROXY` | — | Only if containers can't reach the internet directly (the server must reach Auth0) |

---

## 6. GitHub repository settings (iPhone builds)

| Kind | Name | Needed for |
| --- | --- | --- |
| Variable | `HEALOO_API_BASE_URL` | Server address in iPhone builds; without it they run on demo data |
| Variable | `HEALOO_AUTH0_DOMAIN`, `HEALOO_AUTH0_CLIENT_ID` (Healoo iOS), `HEALOO_AUTH0_AUDIENCE` | Auth0 sign-in in iPhone builds |
| Secret | `IOS_CERTIFICATE_P12_BASE64`, `IOS_CERTIFICATE_PASSWORD`, `IOS_PROVISIONING_PROFILE_BASE64` | Signed build (`build-device`); skipped without them |
| Secret | `APP_STORE_CONNECT_KEY_ID`, `APP_STORE_CONNECT_ISSUER_ID`, `APP_STORE_CONNECT_KEY_P8_BASE64` | TestFlight upload |
| Secret | `GOOGLE_SERVICE_INFO_PLIST_BASE64` | Push notifications in signed builds |

---

## 7. Server components and ports

| Component | Image / version | Port(s) |
| --- | --- | --- |
| care-api / care-worker | built from `Server/Dockerfile` (Rust 1.94, Debian trixie) | LAN 8443 (HTTPS); internet 8080 behind Caddy |
| Cassandra | `cassandra:5.0` | 9042 (inside Docker) |
| Kafka | `apache/kafka:3.8.0` (KRaft, no ZooKeeper) | 9092 inside; LAN also 29092 on the host |
| Kafka UI (LAN only) | `provectuslabs/kafka-ui` | see `deploy/lan/docker-compose.yml` |
| SeaweedFS (internet) | `chrislusf/seaweedfs:4.48`, S3 gateway | 8333 inside Docker |
| Caddy (internet) | `caddy:2` | 80, 443 (API), files host on 443 (or 8444 when run locally) |

Open in the firewall: LAN 8443; internet 80 and 443.

Related: `Configuration.md` (settings), `Administration_Design.md` (dashboard, administrators file,
invite codes, limits), `ServerInstallGuide.txt`, `WUI/README.md` (app builds),
`WUI/android/certs/README.txt` (certificates for debug builds).
