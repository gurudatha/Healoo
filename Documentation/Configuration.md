# Configuration files

Values that used to be constants in the code now live in four files. This page says which
file holds what, how to change it, and what was tested.

| File | Read by | To change a value |
| --- | --- | --- |
| `Server/config/server.json` | Server (API and worker) | Edit, then restart `care-api` and `care-worker`. No rebuild. |
| `Server/config/vitals.json` | Server | Edit, then restart `care-api`. No rebuild. See `Vitals_Design.md`. |
| `WUI/app-config.json` | Android and iOS apps | Edit, then rebuild the apps. |
| `WUI/page-operations.json` | Android and iOS apps | Edit, then rebuild the apps. See `PageOperations_Design.md`. |

Deployment settings (addresses, secrets, sign-in mode) stay in environment variables; see
`Server/README.md`.

## How the files are applied

- **Server files** are compiled into the server as its defaults. The environment variables
  `SERVER_CONFIG` and `VITALS_CONFIG` name files that replace them at start-up.
  - The LAN compose file already mounts `Server/config` and sets both variables, so on the
    trial server editing a file and restarting is enough.
  - The internet compose file (v0.4) mounts `Server/config/server.json` and sets
    `SERVER_CONFIG` the same way; `vitals.json` there still uses the compiled-in copy.
  - Other deployments use the compiled-in values unless they set the variables.
- **A server file with a mistake stops the server at start-up** and prints the reason, so a
  typo can't silently change a limit.
- **App files** are bundled into the apps at build time (Android: the `healooSharedAssets`
  Gradle task; iOS: resources in `project.yml`).
- **Every setting is optional.** One that is left out keeps the default written in the code,
  so a file written for an older version still works. Unknown settings are ignored.
- App values that would break the screens (for example a header taller than the screen) fall
  back to the defaults instead of being used.

## `Server/config/server.json`

| Setting | Default | Meaning |
| --- | --- | --- |
| `uploads.max_attachments` | 20 | Files per item, and per upload request |
| `uploads.image_max_mb` | 10 | Largest image |
| `uploads.pdf_max_mb` | 25 | Largest PDF |
| `uploads.thumbnail_px` | 320 | Longest side of generated thumbnails |
| `uploads.download_link_minutes` | 10 | How long a file link stays valid |
| `messages.max_chars` | 4000 | Longest message |
| `rate_limit.per_second` | 20 | Requests per user per second |
| `rate_limit.burst` | 40 | Short burst allowed above that |
| `appointments.default_timezone` | Asia/Kolkata | Used when none is given (IANA name) |
| `appointments.calendar_days` | 183 | How far ahead visits are prepared for the calendar |
| `appointments.upcoming_visits` | 12 | Upcoming visits listed per appointment |
| `realtime.max_sockets_per_user` | 5 | Live connections per user |
| `administrators.max_platform_admins` | 3 | Most platform administrators (dashboard users) in total, 1–100 |
| `administrators.max_admins_per_hospital` | 2 | Most hospital administrators per hospital, 1–100 |

**The apps follow the upload limits without an update.** `GET /v1/config` returns
`attachments_per_item`, `image_max_bytes`, `pdf_max_bytes` and `message_max_chars`; the apps
read the first three after sign-in. Until then, and in demo mode, they use 20 / 10 MB / 25 MB.
The server enforces its own limits either way.

## `WUI/app-config.json`

| Setting | Default | Meaning |
| --- | --- | --- |
| `layout.home_header` | 0.20 | Share of the screen height of the Home header |
| `layout.person_header_expanded` | 0.25 | Person page header, expanded |
| `layout.person_header_collapsed` | 0.10 | Person page header after scrolling |
| `layout.pinned_header` | 0.10 | Pinned header of the item, upload and other screens |
| `lists.home_preview` | 20 | Open items shown on Home |
| `lists.home_all` | 100 | Open items after "See all" |
| `profile_photo.max_px` | 512 | Profile pictures are resized to this before upload |
| `designations` | Cardiologist, Urologist, Paediatrician, General Medicine | Choices when an administrator adds a doctor |
| `message_cache.max_mb` | 40 | Offline messages; see `OfflineCache_Design.md` |
| `message_cache.trim_mb` | 2 | Oldest data removed when the cache is full |
| `file_cache.max_files` | 15 | Full images and PDFs kept for the viewer |
| `file_cache.max_mb` | 90 | Their total size |

`WUI/design-tokens.json` still lists the header ratios and upload limits as design reference.
The apps do not read it; the files above are what count.

## Still in the code, on purpose

- Protocol and safety limits tied to the code that uses them: request body size (20 MB),
  WebSocket frame size and ping timing, sign-in cache times.
- Names of event topics and database tables.
- The viewer keeps its three newest files regardless of the limits (they may be on screen).

## Tests

### Server — `cargo test --lib` (`settings.rs`)

| Id | Test | Checks |
| --- | --- | --- |
| S1 | `s1_the_shipped_file_holds_the_values_the_code_used_to_have` | Moving the values changed nothing |
| S2 | `s2_editing_the_file_changes_the_limits` | An edited file changes behaviour |
| S3 | `s3_a_partial_or_older_file_keeps_defaults_for_the_rest` | Optional settings, unknown sections |
| S4 | `s4_a_broken_file_is_refused_with_the_reason` | Bad time zone, zero sizes, burst below rate |

### Android — `gradlew :app:testDebugUnitTest` (`AppConfigTest`)

| Id | Test | Checks |
| --- | --- | --- |
| K1 | `k1_the_shipped_file_holds_the_values_the_code_used_to_have` | Moving the values changed nothing |
| K2 | `k2_editing_the_file_changes_the_app` | Header share, list size, designations |
| K3 | `k3_values_that_would_break_the_screens_fall_back` | Unsafe values are not used |
| K4 | `k4_upload_limits_follow_the_server` | Limits from `/v1/config`; older servers and bad values |

**Results, 3 Oct 2026:** server 45 passed, 0 failed (S1–S4 among them), and `cargo build --bins`
succeeded. Android 32 passed, 0 failed (K1–K4 among them), and the app compiles. iOS reads
the same file with the same defaults but **has not been compiled** (no Mac available). The
running trial server and a phone were not used for this change.
