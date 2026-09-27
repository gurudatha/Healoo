# Administration — design specification

Status: **implemented** (server v0.3, apps 0.5). The server is built, unit-tested and checked
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

These make up **access rule 10** (design document section 2.3).

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
