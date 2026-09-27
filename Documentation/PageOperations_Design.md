# Page operations — design specification

Status: **implemented** (server v0.3, apps 0.5). Server and Android are built and unit-tested;
iOS is written but not yet compiled (see 7.4).

## 1. Idea in one paragraph

The bottom bar shows what can be done **on the page you are on**. On a person's page every
operation acts on that person: a message goes to them, an upload is for them, a share is with
them. They are always a recipient and can't be removed. More people can be added with the
**Filtered Search Bar**. Pages that are not about a person (Home, Search, Upload, Messages,
Settings) show the global operations. A bar shows at most four operations; the rest sit behind
a three-dot **More** button.

## 2. Decisions

| # | Decision |
| --- | --- |
| D1 | The mapping of pages to operations lives in **one file**, `WUI/page-operations.json`, which both apps bundle and read at start-up. What each operation *does* stays in code, keyed by id. |
| D2 | At most `maxVisible` (4) operations are shown; the rest go behind **More**. The global page follows the same rule (Settings moves to More). |
| D3 | The page kind is chosen from the person's primary role: DOCTOR → doctor page, HOSPITAL → hospital page, anyone else → user page. |
| D4 | On a person's page, that person is the fixed recipient of Message, Share and Upload. Extra recipients are picked with the Filtered Search Bar (no checkbox lists). |
| D5 | **Share a document** shares one of the viewer's *existing* items. New documents only come in through **Upload**, either into a new item or into an existing one. |
| D6 | History is not a separate screen: it is the person page itself (Shared items / Messages). The History operation switches back to it. |
| D7 | Access rule 9 (section 5): a doctor, lab or hospital may pass an item it can fully read on to a **doctor** (referral), even without owning it. Hospitals, like labs, can upload for a patient in their contacts. |

## 3. The configuration file

`WUI/page-operations.json`:

```json
{
  "maxVisible": 4,
  "pageForRole": { "DOCTOR": "doctor", "HOSPITAL": "hospital", "*": "user" },
  "operations": {
    "message": { "label": "Message", "icon": "chat", "requires": ["connected"] },
    "...": {}
  },
  "pages": {
    "global":   ["home", "search", "upload", "messages", "settings"],
    "user":     ["connect", "message", "history", "share_document", "upload_for"],
    "doctor":   ["connect", "message", "history", "share_document", "book_appointment", "upload_for"],
    "hospital": ["search_doctors", "book_appointment"]
  }
}
```

- **Order in `pages` is the order on the bar.** The first `maxVisible` operations whose
  `requires` all hold are shown, and the rest go into More.
- **`requires`** can be: `connected`, `not_connected` (the page's person), `viewer_patient`,
  `viewer_clinical` (doctor, assistant, lab, hospital).
- **`icon`** is a key each app maps to its own icon set: `ActionBar.kt` on Android,
  `OperationBar.swift` on iOS.
- **Unknown operation ids are ignored,** so a newer file can't break an older app.
- **Bundling:**
  - Android: the `healooSharedAssets` Gradle task copies the file into the APK's assets.
  - iOS: `project.yml` lists it as a resource.

To change the bar, edit only this file and rebuild both apps.

## 4. Operations

| Page | Visible (in order) | More | What each does |
| --- | --- | --- | --- |
| Global | Home, Search, Upload, Messages | Settings | Switch screen |
| User (connected) | Message, History, Share, Upload | — | See below |
| Doctor (connected) | Message, History, Share, Book | Upload | See below |
| User / doctor, not connected | Add contact (+ Book on a doctor) | — | Adds to contacts; the rest appears once connected |
| Hospital | Doctors, Book | — | See below |

| Operation | Behaviour |
| --- | --- |
| Message | Sheet: recipients (page person fixed + Filtered Search Bar) and the text. Creates a MESSAGE item shared with all recipients and opens its discussion. A clinician writing to a patient creates it on the patient's behalf (the patient owns it) and can add doctors only. |
| History | Shows the page's Shared items / Messages (D6). |
| Share | Sheet: pick one of the viewer's open items (Filtered Search Bar with Reports / Appointments / Messages / Alerts filters), recipients as above, then **Share**. If the viewer doesn't own the item, only doctors can be recipients (rule 9). |
| Upload | Opens Upload with the page person as the fixed recipient. A clinician on a patient's page uploads *for* the patient (patient locked as owner). Upload offers **New item / Existing item**; Existing lists the items shared with that person. |
| Book (doctor page) | Upload in Appointment mode with this doctor chosen. |
| Book (hospital page) | Sheet: pick one of the hospital's doctors (Filtered Search Bar), then the appointment form with `hospital_id` set. |
| Doctors (hospital page) | Moves focus to the Filtered Search Bar over the hospital's doctors, which the hospital page lists. |

**Filtered Search Bar:** a reusable component (`FilteredSearchBar.kt`, `OperationBar.swift`).

- **Search field:** a text field with optional filter chips (All, Doctors, Hospitals, Labs, Users).
- **Candidates:** the viewer's contacts are filtered on the device. Server search
  (`/v1/search`) adds matches once two or more characters are typed.
- **Picking a result:** adds it as a removable chip.

## 5. Access rule 9 (addition to design doc 2.3)

| Who adds the grant | To whom | Allowed |
| --- | --- | --- |
| Owner | Anyone (user or hospital) | Yes |
| Doctor, lab or hospital with **full** access | A doctor (user grant) | Yes (referral) |
| Doctor, lab or hospital | Patient, lab, hospital, or a doctor as a hospital grant | No |
| Doctor, lab or hospital with metadata-only access | Anyone | No |
| Patient or assistant who is not the owner | Anyone | No |
| Someone creating an item **for a patient** (`owner_id` ≠ caller) | `share_with` doctors only | Yes; others → 403 |

- **Revoking:** only the owner revokes. The owner sees referral grants in the item's access
  list and in Active sharing.
- **Rule 2 extended:** a lab *or hospital* sees the items it uploaded for a patient.
- **Server code:** `policy::may_share`, `policy::may_share_on_create` (`Server/care-api/src/policy.rs`),
  used by `POST /v1/grants` and `POST /v1/items`. `allowed_actions` includes `share` for doctors,
  labs and hospitals with full access.

## 6. API changes

| Method | Path | Change |
| --- | --- | --- |
| GET | `/v1/hospitals/{id}/doctors` | **New.** Doctors with an active affiliation, profiles only, sorted by name; `connected` is from the caller's view. 400 if `{id}` is not a hospital. |
| POST | `/v1/grants` | Rule 9: non-owners may refer to doctors (403 with the reason otherwise). |
| POST | `/v1/items` | Hospitals may create for a patient in their contacts. Creating for a patient now honours `share_with` (doctors only). |
| POST | `/v1/items` (appointment) | Apps now send `hospital_id` when booking from a hospital's page (field already existed). |

## 7. Tests

### 7.1 Server unit tests — `cargo test --lib` (Server/care-api)

| Id | Test (`policy.rs`) | Checks |
| --- | --- | --- |
| P1 | `p1_hospital_upload_visible_to_hospital_and_patient` | Rule 2 extended to hospitals |
| P2 | `p2_owner_shares_with_anyone` | Owner → patient, doctor, hospital, lab |
| P3 | `p3_doctor_lab_and_hospital_refer_to_a_doctor` | Referral allowed for each referring role |
| P4 | `p4_referral_only_to_doctors_and_only_with_full_access` | Non-doctor grantees, hospital grants and metadata-only access refused |
| P5 | `p5_patients_and_assistants_cannot_share_what_they_do_not_own` | Non-owner patient / assistant refused |
| P6 | `p6_creating_for_a_patient_shares_with_doctors_only` | `share_with` when creating for a patient |

**Result, 26 Sep 2026:** 18 passed, 0 failed. That is P1–P6 plus the existing 12 (acceptance
tests 2–7 and recurrence). `cargo build --bins` succeeded. Run in the `rust:1.94-slim` Docker
image.

### 7.2 Android unit tests — `gradle :app:testDebugUnitTest` (WUI/android)

These run against the real `WUI/page-operations.json`, so they also guard the configuration
file.

| Id | Test (`PageOperationsTest.kt`) | Checks |
| --- | --- | --- |
| B1 | `b1_global_page_shows_four_then_more` | Home, Search, Upload, Messages; Settings in More |
| B2 | `b2_user_page_is_for_that_user` | Message, History, Share, Upload; nothing in More |
| B3 | `b3_doctor_page_adds_booking_and_moves_the_fifth_to_more` | Message, History, Share, Book; Upload in More |
| B4 | `b4_hospital_page_searches_doctors_and_books` | Doctors, Book |
| B5 | `b5_not_connected_offers_add_contact_first` | Add contact (and Book on a doctor page) only |
| B6 | `b6_role_decides_the_page` | DOCTOR / HOSPITAL / others → page kinds |
| B7 | `b7_never_more_than_max_visible` | No page shows more than 4; More appears only when the bar is full |

**Result, 26 Sep 2026:** 7 passed, 0 failed. `gradle :app:assembleDebug` succeeded, and the APK
contains `assets/page-operations.json`.

### 7.3 Manual checks — TestingReadMe.txt section 4, steps 22–33

- **What they cover:** the screens themselves (sheets, chips, Filtered Search Bar,
  New / Existing item, hospital booking) in demo mode.
- **Status, 26 Sep 2026: not run yet.** The emulator was offline during this change. Run them
  before release and record the result here.

### 7.4 iOS

- **Status:** the iOS changes mirror Android: `OperationBar.swift`, `UserPageAndThreads.swift`,
  `UploadAndSettings.swift`, `HealooApp.swift`, `Repository.swift`, `project.yml`. They
  **have not been compiled**, because there was no Mac.
- **Before relying on them:** run `xcodegen generate`, build, and repeat the manual checks in 7.3.

## 8. Files

| Area | Files |
| --- | --- |
| Config | `WUI/page-operations.json` |
| Server | `policy.rs` (rule 9, tests), `api/grants.rs`, `api/items.rs`, `api/users.rs` (hospital doctors), `api/mod.rs`, `model.rs` (`is_clinical` + hospital, `can_refer`) |
| Android | `data/PageOperations.kt`, `ui/components/ActionBar.kt`, `ui/components/FilteredSearchBar.kt`, `ui/user/UserPageScreen.kt`, `ui/upload/UploadScreen.kt`, `HealooApp.kt`, repositories, `app/build.gradle.kts`, `src/test/.../PageOperationsTest.kt` |
| iOS | `Components/OperationBar.swift`, `Features/UserPageAndThreads.swift`, `Features/UploadAndSettings.swift`, `App/HealooApp.swift`, `Data/Repository.swift`, `Models/Models.swift`, `project.yml` |
