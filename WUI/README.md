# Healoo WUI — Android (Compose) and iOS (SwiftUI)

These are native client apps for the **Sage Clinic** theme and the behaviour set out in the *Healoo Design Document*. Both apps start with seeded fake data, so every screen works before the backend runs.

```
healoo-wui/
├── design-tokens.json        colours, type, radii, header ratios, limits (shared source of truth)
├── android/                  Kotlin + Jetpack Compose, minSdk 29
└── ios/                      Swift + SwiftUI, iOS 17, generated with XcodeGen
```

> **Build status (0.3):** this code was written without access to Gradle or Xcode, so it has **not been compiled**. Expect a few small fixes on the first build, especially around third-party SDK versions (Auth0, Firebase, ML Kit). Every file is self-contained and commented.

---

## 1. Fonts (required, both platforms)

Download **Fraunces** and **Figtree** from fonts.google.com and use the files in their `static/` folders.

| Use | Android (`android/app/src/main/res/font/`) | iOS (`ios/Healoo/Resources/Fonts/`) |
|---|---|---|
| Display | `fraunces_semibold.ttf` | `Fraunces-SemiBold.ttf` |
| Body | `figtree_regular.ttf` | `Figtree-Regular.ttf` |
| Body medium | `figtree_medium.ttf` | `Figtree-Medium.ttf` |
| Body strong | `figtree_semibold.ttf` | `Figtree-SemiBold.ttf` |

- **Android:** rename the files to the lower-case names shown in the table. The build fails if any file is missing.
- **iOS:** the file names must match `UIAppFonts` in `project.yml`. The PostScript names are in `Theme.swift → FontName`. If Font Book shows a different name (for example `Fraunces72pt-SemiBold`), change it there. A wrong name does not crash the app; the text just shows in the system font.

## 2. Android

1. Open `android/` in Android Studio (Koala or newer). Let it create the Gradle wrapper and sync.
2. Add the fonts (section 1).
3. Run on an emulator (API 29+).

**Fake or real backend.** The settings are in `android/gradle.properties`:
- In `gradle.properties`, `healoo.useFakeData=true` uses the seeded data, demo accounts and bundled sample files. This is the default.
- Set it to `false` to call the local stack at `API_BASE_URL = https://10.0.2.2/`, which is Caddy/Nginx on your host (doc 9.4). Install the Caddy/mkcert root CA on the emulator. The debug network config trusts user CAs; the release config does not.


## 3. iOS

1. `brew install xcodegen`, then `cd ios && xcodegen generate`.
2. Add the fonts (section 1), then open `Healoo.xcodeproj`.
3. Set your signing team and run on an iOS 17+ simulator.

**Fake or real backend.** Edit `HealooUseFakeData` and `HealooAPIBaseURL` in `project.yml`, then run `xcodegen` again. For the real stack, trust the local root CA in the simulator by dragging `root.crt` onto it and enabling it under Settings → General → About → Certificate Trust Settings.

The camera needs a real device. On the simulator the Camera button explains this, and Images and PDFs still work.

## 4. Screens → design document

| Screen | Android | iOS | Doc |
|---|---|---|---|
| Landing: fixed 20% header, counters, search, open items | `ui/landing` | `LandingAndSearch.swift` | 3.2 |
| Search: by ID or name, profiles only, Add to contacts | `ui/search` | `LandingAndSearch.swift` | 2.3 #8, 3.4 |
| User page: header collapses from 25% to a pinned 10%; shared items and messages | `ui/user`, `CollapsingHeader.kt` | `UserPageAndThreads.swift` | 3.3 |
| Data view: pinned 10% header, attachments, keywords, links, access list with revoke, linked item, share | `ui/item` | `DataItemAndViewer.swift` | 3.5 |
| Attachment viewer: swipe across all files, PDF pages scroll vertically, pinch/double-tap zoom, "2 / 5", thumbnail strip | `ui/viewer` | `DataItemAndViewer.swift` | 3.6 |
| Upload: several images and PDFs together, camera, links, reorder/remove, 20 files / 10 MB image / 25 MB PDF limits, share, link to message | `ui/upload` | `UploadAndSettings.swift` | 3.5, 4.5 |
| Settings | `ui/settings` | `UploadAndSettings.swift` | 3.x |
| Messages list | `ui/threads` | `UserPageAndThreads.swift` | 4.x |

**Behaviour notes**
- Header sizes are fractions of the screen height (`HeaderRatio`), with the status bar added on top, so buttons are never clipped.
- **Zoom:** in the viewer, a zoomed image pans instead of swiping. Double-tap resets the zoom, and then swiping moves to the next file.
- **Upload:** files are presigned in one batch, uploaded with PUT directly to storage, and then the item is created. After a successful upload, the new item opens.

**API fields the clients expect.** These are display fields in addition to the doc's DataItem, used for list rows:
- DataItem: `title`, `subtitle`, `created_by_name`
- Profile: `headline`
- Search and connection rows: `connected`

The server can fill these from the denormalised tables (`items_by_owner`, `users_by_public_id`). Add them to doc section 4.3 if you want the contract written down.

## 5. Added in 0.2

### Demo accounts (fake-data mode)
With fake data on (the default), the sign-in screen lets you choose an account:

| Account | Use it to test |
|---|---|
| **Lakshmi K.** (patient) | Sharing, revoking, Active sharing, uploads, editing the profile |
| **Dr. Anitha Rao** (doctor) | Seeing items shared with you or your hospital, messaging the patient, "Upload for Lakshmi" |
| **City Diagnostics** (lab) | Uploading a report for a patient (Lakshmi or Priya) |

To switch accounts, go to Settings → Log out. The demo data is kept while the app is running, so an upload made as the lab shows up when you sign in as the patient. When you send a message, a simulated reply arrives about 1.5 seconds later through the live-events path.

### Auth0 sign-in and log out (doc 5)
- **Android:** set `healoo.useFakeData=false`, `healoo.auth0.domain`, `healoo.auth0.clientId` and `healoo.auth0.audience` in `gradle.properties` (or in `~/.gradle/gradle.properties` to keep them out of git).
- **iOS:** set the same values as the `HealooAuth0*` keys in `project.yml`, then run `xcodegen` again.
- In the Auth0 application (type *Native*), add these callback and logout URLs:
  - Android: `healoo://<domain>/android/com.healoo.app/callback`
  - iOS: `com.healoo.app://<domain>/ios/com.healoo.app/callback`
- Enable the *Refresh Token* grant, because the apps request `offline_access`.
- Sign-in uses Universal Login with PKCE. Tokens are stored by the Auth0 SDK (Android: SharedPreferences; iOS: Keychain).
- A 401 response triggers one silent refresh and then a retry. Log out clears the Auth0 session, removes this device from push, closes the WebSocket and returns to the sign-in screen.

### Push notifications (doc 8)
- **Android:** put `google-services.json` in `android/app/`. The Google Services plugin is applied only when that file exists, so builds without it still work, just without push.
- **iOS:** add `GoogleService-Info.plist` to `ios/Healoo/` and upload your APNs key (.p8) to Firebase. The Push Notifications capability comes from `project.yml`. Push needs a real device.
- After sign-in, the app asks for notification permission, gets the FCM token and registers it with `POST /v1/devices`. On log out it calls `DELETE /v1/devices/{token}`.
- Payload the backend should send (data message): `type` (`message` | `report` | `alert` | `booking`), `title`, `body`, and optionally `item_id` or `user_id`. Tapping the notification opens that item or conversation.
- Android uses two channels: *Messages & alerts* (high importance) and *New reports & bookings*.

### Live messages (WebSocket, doc 4.4)
- The apps connect to `wss://<host>/v1/ws` with `Authorization: Bearer <token>` while in the foreground, and reconnect with backoff from 1 to 30 seconds.
- Frames handled:
  - `{"type":"message.new","message":{…}}`: appended to the open conversation, and the Messages list refreshes.
  - `{"type":"item.updated","item_id":"…"}`: Home refreshes.
  - `{"type":"ping"}`: the app answers `{"type":"pong"}`.
- Sending a message still uses REST with a `client_msg_id`, so a message that also comes back over the socket isn't shown twice.

### Healoo ID QR
- **Scanning:** the QR button in Search scans a code and opens that person's profile. It shows the profile only; records still need a grant. Android uses Google's code scanner, which needs no camera permission. iOS uses VisionKit and needs a real device.
- **Your own code:** Edit profile shows your QR, which encodes `healoo://id/HL-XXXXX`. A plain `HL-XXXXX` also scans.

### Edit profile
- You can change your display name and location (`PATCH /v1/me`).
- For doctors, the registration number and current hospital are shown read-only, because the hospital administrator sets them.

### Active sharing
- Settings → Active sharing lists everything you've shared, grouped by person or hospital.
- You can revoke one item or use "Stop all" for a person or hospital. Both ask for confirmation.
- The list comes from `GET /v1/grants?owner=me`.

### Notification settings
- The three toggles are saved to `PUT /v1/me/notification-prefs` and loaded from `GET /v1/me/notification-prefs`.
- The change shows immediately and is undone if the server refuses it.

### Uploading for a patient (doctor, assistant and lab)
- On a connected patient's page, **Upload for <name>** opens Upload with that patient locked in. Opening the Upload tab directly as a clinician shows a patient picker instead.
- The patient owns the new item. The uploader keeps access automatically, and the Share section is hidden, because only the owner decides sharing (doc 2.3).
- The server should enforce the same rule: `POST /v1/items` with `owner_id` ≠ caller is allowed only for clinical roles connected to that patient, and it adds a grant for the uploader.
- When a doctor opens a patient's item, the action button reads **Message patient**.

### New or changed API endpoints the apps call
Please add these to design doc section 4.3:

| Method and path | Body or response |
|---|---|
| `PATCH /v1/me` | `{display_name, location}` → profile |
| `GET / PUT /v1/me/notification-prefs` | `{push_messages, push_reports, quiet_hours, quiet_start, quiet_end}` |
| `GET /v1/grants?owner=me` | page of `{grant_id, grantee_type, grantee_id, grantee_name, item_id, item_title, core_item_type}` |
| `POST /v1/devices` | `{platform: "android" \| "ios", token}` |
| `DELETE /v1/devices/{token}` | — |
| `GET wss://…/v1/ws` | frames listed above |

## 6. Added in 0.3: developer sign-in (trial server)

Sign in to the **healoo-backend** trial server without Auth0, using its seeded test accounts
(backend running with `NETWORK_MODE=lan` and `AUTH_MODE=dev`).

- **Android** (`gradle.properties`): `healoo.useFakeData=false`,
  `healoo.apiBaseUrl=https://<computer's LAN IP>:8443/` (the emulator can use `https://10.0.2.2:8443/`),
  `healoo.devSignIn=true` (the default).
- **iOS** (`project.yml`): `HealooUseFakeData: false`, `HealooAPIBaseURL: https://<LAN IP>:8443/`,
  `HealooDevSignIn: true`, then `xcodegen generate`.
- Android debug builds bundle the backend's `healoo-local-ca.crt` automatically (from `../../healoo-backend/deploy/lan/`, `android/certs/`, or `healoo.trustedCaFile`); no device install needed. iOS: install it on the simulator/phone.
- The sign-in screen then shows **Developer sign-in**: the server's test accounts (from `GET /dev/users`)
  and a field for any Healoo ID. Tapping one calls `POST /dev/token` and continues as normal.
- The Healoo ID is remembered. The app restores the session on launch, and when the 12-hour token
  expires it gets a new one silently (after a 401, or when the WebSocket is closed with 4001).
- Log out forgets the account. The Auth0 button still appears when Auth0 is configured, so both can be used.
- **Safety:** release builds have it compiled out (Android forces `DEV_SIGN_IN=false` in the release
  build type; iOS wraps it in `#if DEBUG`). The backend only offers `/dev/*` in LAN mode with dev auth.

Also in 0.3: both apps renew the token and reconnect when the server closes the live channel
with code 4001 (token expired), for Auth0 sessions too.

## 7. Added in 0.3: app icon

The icon is design-canvas version **C** (stethoscope clock with four clockwise arrows).
Sources live in `branding/` (`healoo-icon.svg` square icon, `healoo-mark.svg` mark only,
`healoo-mark-mono.svg` one colour, `healoo-icon-1024.png`).

- **Android:** adaptive icon in `res/mipmap-anydpi-v26/` (background colour + vector foreground,
  plus a monochrome layer for Android 13 themed icons). `ic_launcher-playstore.png` is the 512 px
  Play Store icon.
- **iOS:** `Resources/Assets.xcassets/AppIcon.appiconset` (one 1024 px image; Xcode makes the
  other sizes) and `HealooMark` for in-app use.
- The sign-in header shows the mark next to the name on both platforms.

## 8. Build tools (updated in 0.3)

Gradle 9.3.0, Android Gradle plugin 9.0.1, JDK 17, compile SDK 36 (target SDK stays 35).
Gradle 9 does not run on JDK 11, so make sure `./gradlew --version` reports JDK 17 for both the
launcher and the daemon (TestingReadMe.txt section 1 shows how). AGP 9 compiles Kotlin itself,
so the project no longer applies the `org.jetbrains.kotlin.android` plugin.

## 9. Sample data

`assets/sample/` (Android) and `Resources/Sample/` (iOS) hold two scan images and two PDFs, with 3 and 2 pages. Each file is marked **SAMPLE TEST DATA — not a real medical report**. They belong to the seeded "CBC – Complete blood count" item, so the viewer can be tested straight away (tests 13–14).
