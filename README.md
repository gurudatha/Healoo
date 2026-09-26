# Healoo

Healoo (working name: CareConnect) connects patients, doctors, labs and hospitals. Patients own
their medical records (reports, appointments, messages, alerts) and decide exactly who can see
each one. Doctors, labs and hospitals see only what has been shared with them.

> **Status: trial / development.** Use test data only. Never enter real patient information
> until the production checklist in the design document (section 10.6) has been completed.

## Repository layout

| Folder | What it contains |
| --- | --- |
| [`Server/`](Server/) | Rust backend `care-api`: REST + WebSocket API, Auth0 / developer sign-in, Kafka events, Cassandra storage, file storage. Runs on a local network for trials or on the internet behind Caddy/Nginx. |
| [`WUI/`](WUI/) | Mobile apps: Android (Kotlin, Jetpack Compose) and iOS (SwiftUI), "Sage Clinic" design, demo mode with sample data, app icon and branding files. |
| [`Documentation/`](Documentation/) | Design document, install and testing guides, API documentation, library list, app icon. Start with its [README](Documentation/README.md). |

## Quick start

1. **Run the server** on a local network: [`Documentation/ServerInstallGuide.txt`](Documentation/ServerInstallGuide.txt)
   (Docker Desktop, then `docker compose up -d --build` in `Server/deploy/lan`).
2. **Run the Android app** on Windows: [`Documentation/WUI_Windows_Guide.txt`](Documentation/WUI_Windows_Guide.txt)
   (JDK 17, Android SDK, emulator, `.\gradlew.bat installDebug` in `WUI/android`).
3. **Test**: [`Documentation/TestingReadMe.txt`](Documentation/TestingReadMe.txt) and the API calls in
   [`Documentation/userapidocumentation.txt`](Documentation/userapidocumentation.txt).

The guides were written when the two parts were separate downloads. Where they mention
`healoo-backend` use `Server/`, and for `healoo-wui` use `WUI/`.

## Technology

| Part | Stack |
| --- | --- |
| Server | Rust, Tokio, axum (REST + WebSocket), Auth0 JWT, Apache Kafka, Apache Cassandra 5, S3/MinIO or local disk for files, Docker Compose |
| Android | Kotlin, Jetpack Compose, Gradle 9.3.0, Android Gradle plugin 9.0.1, JDK 17, min SDK 29, target SDK 35 |
| iOS | Swift, SwiftUI, iOS 17+, XcodeGen |

## Not included in the repository

These are created on each machine and are ignored by Git (see `.gitignore`):

- `Server/deploy/*/.env` (secrets), server data and generated certificates
- `WUI/android/local.properties`, the Gradle wrapper jar, build folders
- The app fonts (Fraunces, Figtree): download them as described in the Windows guide, section 5
- Signing keys (`*.jks`) and your personal `~/.gradle/gradle.properties`
