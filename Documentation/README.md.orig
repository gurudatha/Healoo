# Healoo documentation

| File | What it is | Read it when |
| --- | --- | --- |
| [`Healoo_Design_Document.docx`](Healoo_Design_Document.docx) | The system design: roles and access rules, screens, API and WebSocket protocol, Auth0, Kafka topics, Cassandra schema, push notifications, proxies, local trial and acceptance tests. | You need to understand how the system is meant to work, or before changing its behaviour. |
| [`ServerInstallGuide.txt`](ServerInstallGuide.txt) | Installing the server and everything it depends on (Docker, Cassandra, Kafka) on Windows, macOS or Ubuntu, starting from a bare machine. Includes building natively in WSL and troubleshooting. | Setting up the server for the first time. |
| [`WUI_Windows_Guide.txt`](WUI_Windows_Guide.txt) | Installing JDK 17, the Android SDK and Gradle on Windows; running the app on the emulator; building debug and signed release APKs. | Building or testing the Android app on Windows. |
| [`TestingReadMe.txt`](TestingReadMe.txt) | Running both apps (Android from VS Code, iOS on a Mac), a step-by-step test checklist, and testing against the trial server with developer sign-in. | Testing the apps. |
| [`userapidocumentation.txt`](userapidocumentation.txt) | Developer reference for the REST API: authentication (Auth0 and developer tokens), adding users, searching users, uploading documents, listing data items, with curl examples. | Calling the API from Postman, curl or another client. |
| [`RustLibraries.txt`](RustLibraries.txt) | Three-line description of every Rust library in the server build, grouped by purpose. | Reviewing dependencies or investigating a build problem. |
| [`healoo-icon-1024.png`](healoo-icon-1024.png) | The app icon (stethoscope clock with four clockwise arrows), 1024 × 1024. Source files are in `WUI/branding/`. | Store listings, website, presentations. |

## Folder names in the guides

The guides were written when the server and apps were separate downloads:

- `healoo-backend` = [`../Server`](../Server)
- `healoo-wui` = [`../WUI`](../WUI)

## Changes since the design document was exported

The Word document is an export of the design as it stood before implementation started.
Since then:

- **Server implemented (v0.1):** sections 4–7. See `Server/README.md` for schema changes (for
  example, the `grant` type is named `item_grant` because GRANT is reserved in CQL) and for what
  is not built yet (push-notification sender, alert scheduler, paging).
- **Developer sign-in:** debug builds of the apps can sign in to the local trial server with its
  test accounts, without Auth0.
- **Android build:** Gradle 9.3.0, Android Gradle plugin 9.0.1, JDK 17, compile SDK 36.
- **Trial-server certificate:** Android debug builds bundle it automatically (see
  `WUI/android/certs/README.txt`).
- **App icon:** added to both apps (design version C).
- **Proposed, not implemented:** a DataItem that works as a container for one case (a primary
  appointment, message, alert or report, plus messages, attachments, appointments with
  recurrence, alerts, and closing feedback and rating).
