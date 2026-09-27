# Healoo documentation

| File | What it is | Read it when |
| --- | --- | --- |
| [`Healoo_Design_Document_v2.docx`](Healoo_Design_Document_v2.docx) | **The design document (version 2, 27 September 2026).** Part 1: user requirements — what the app shows, how it behaves, lists items and sends messages, and every user operation. Part 2: implementation — for the apps and the servers separately: minimum requirements, memory, request capacity, when data is fetched and refreshed, how it is stored and cached. | Start here: understanding what the app must do, or how it is built and what it needs to run. |
| [`Healoo_Design_Document.docx`](Healoo_Design_Document.docx) | Version 1 (superseded by version 2), kept for history. The original system design: roles and access rules, screens, API and WebSocket protocol, Auth0, Kafka topics, Cassandra schema, push notifications, proxies, local trial and acceptance tests. | You need to understand how the system is meant to work, or before changing its behaviour. |
| [`DataItem_Design.md`](DataItem_Design.md) | **Implemented** DataItem v2: an item as a container for one case (primary appointment, message, alert or report, plus messages, attachments, recurring appointments, alerts, closing feedback and rating). Fields, validation, recurrence rules, API, Cassandra tables, Rust/Kotlin types, migration. | Before implementing or reviewing the new item model. |
| [`PageOperations_Design.md`](PageOperations_Design.md) | **Implemented** bottom-bar operations per page (user, doctor, hospital, global) from one config file, `WUI/page-operations.json`; the Filtered Search Bar; access rule 9 (referral to a doctor, hospital uploads); the new API; and the tests with their results. | Changing what a page offers, or reviewing sharing rules. |
| [`Administration_Design.md`](Administration_Design.md) | **Implemented** hospital administration (access rule 10): administrators add users and doctors (with a designation), delete doctors by deactivating them and can reactivate them. Users can never be deleted. Covers the API, the database columns, the Settings → Administration screens and the tests. | Managing accounts, or reviewing who may create or remove them. |
| [`ServerInstallGuide.txt`](ServerInstallGuide.txt) | Installing the server and everything it depends on (Docker, Cassandra, Kafka) on Windows, macOS or Ubuntu, starting from a bare machine. Includes building natively in WSL and troubleshooting. | Setting up the server for the first time. |
| [`WUI_Windows_Guide.txt`](WUI_Windows_Guide.txt) | Installing JDK 17, the Android SDK and Gradle on Windows; running the app on the emulator; building debug and signed release APKs. | Building or testing the Android app on Windows. |
| [`TestingReadMe.txt`](TestingReadMe.txt) | Running both apps (Android from VS Code, iOS on a Mac), a step-by-step test checklist, and testing against the trial server with developer sign-in. | Testing the apps. |
| [`userapidocumentation.txt`](userapidocumentation.txt) | Developer reference for the REST API: authentication (Auth0 and developer tokens), adding users (including administrator-created users and doctors), searching users, uploading documents, listing data items, with curl examples. | Calling the API from Postman, curl or another client. |
| [`RustLibraries.txt`](RustLibraries.txt) | Three-line description of every Rust library in the server build, grouped by purpose. | Reviewing dependencies or investigating a build problem. |
| [`healoo-icon-1024.png`](healoo-icon-1024.png) | The app icon (stethoscope clock with four clockwise arrows), 1024 × 1024. Source files are in `WUI/branding/`. | Store listings, website, presentations. |

## Folder names in the guides

The guides were written when the server and apps were separate downloads:

- `healoo-backend` = [`../Server`](../Server)
- `healoo-wui` = [`../WUI`](../WUI)

## Change history

Version 2 of the design document (Healoo_Design_Document_v2.docx, 27 September 2026) is the current
one; it reorganises everything into user requirements and implementation. Version 1 below:
the Word document was first exported before implementation started. On 27 September 2026 it
was revised to match the implementation (server v0.3, apps 0.5), including the changes below:

- **Server implemented (v0.1):** sections 4–7. See `Server/README.md` for schema changes (for
  example, the `grant` type is named `item_grant` because GRANT is reserved in CQL) and for what
  is not built yet (push-notification sender, paging). The alert scheduler was added in v0.2.
- **Developer sign-in:** debug builds of the apps can sign in to the local trial server with its
  test accounts, without Auth0.
- **Android build:** Gradle 9.3.0, Android Gradle plugin 9.0.1, JDK 17, compile SDK 36.
- **Trial-server certificate:** Android debug builds bundle it automatically (see
  `WUI/android/certs/README.txt`).
- **App icon:** added to both apps (design version C).
- **DataItem v2 implemented (server v0.2, apps 0.4):** an item is a container for one case —
  see [`DataItem_Design.md`](DataItem_Design.md). It replaces the item model in design document
  section 2.2 and the separate chat threads: every message now belongs to an item, and the
  Messages tab lists conversations per item. Includes recurring appointments, alerts with a
  scheduler, and closing with feedback and a patient-only rating. The trial database must be
  recreated (`docker compose down -v`) and re-seeded.
- **Page operations (server v0.3, apps 0.5):** the bottom bar shows the operations of the page
  you are on, from `WUI/page-operations.json` (at most four, the rest under More). Adds access
  rule 9 to section 2.3: doctors, labs and hospitals may pass an item on to a doctor, and
  hospitals may upload for a patient. See [`PageOperations_Design.md`](PageOperations_Design.md),
  including which tests have run. No database change.
- **Administration and more trial accounts (server v0.3, apps 0.5):** hospital administrators
  add users and doctors, and delete doctors by deactivating them (Settings → Administration).
  Users can never be deleted. Adds access rule 10 to section 2.3. See
  [`Administration_Design.md`](Administration_Design.md). New `users` columns and a
  `users_by_email` table are added in place, so the trial database doesn't need recreating. The
  seed job adds nine accounts (3 patients, 4 doctors with designations, 2 labs) to existing
  trial databases.
- **Landing page and navigation (apps 0.5):** the dashboard tiles filter the open items (the
  selected tile is highlighted; tap again to clear), "See all" expands the list in place, Home
  always returns to the landing page, and every list that can outgrow the screen shows a scroll
  bar (including the developer sign-in list). Design document sections 3.2, 3.7 and 3.8.
