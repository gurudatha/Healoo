# Vitals — design specification

Status: **implemented** on the server and Android (built and unit-tested). iOS is written but
not compiled, and the screens have not been walked through on a device (see section 8).

## 1. Idea in one paragraph

A user records small, frequent measurements (water, BP, sugar, heart rate, urine, weight and
so on) straight from Home. Each reading is coloured **green, yellow or red** against that
person's own range. The doctors in the person's contacts see the readings consolidated on
their Home as red / yellow / green alerts, can swipe an alert away or open the patient, set
the patient's ranges and add vitals for them. Vitals are **not DataItems**: they have their
own data structures, tables and API.

## 2. Decisions

| # | Decision |
| --- | --- |
| V1 | **Readings are stored per user**, not per vital. One person's history, latest values and sorting are single reads, and the data stays with its owner. A per-vital layout would put every patient's BP in one ever-growing partition and mix patients' data. |
| V2 | Each doctor has a separate **alert inbox**. A new reading writes one entry into the inbox of every doctor in the person's contacts. Read state is per doctor: one doctor swiping an alert away doesn't hide it from another. |
| V3 | **Colour rule:** inside the range is green; outside by up to the margin (30 % of the bound that was crossed) is yellow; beyond that is red. A vital with several values (BP) takes the worst. |
| V4 | Every person starts with the **default range** of each vital. **Only a doctor** in the patient's contacts can change a range or the margin, per patient. Ranges are never sent to the patient; they see only the colour. |
| V5 | **Normal readings also reach the doctor**, as unread green entries. Only red and yellow ask for a push notification. |
| V6 | The catalogue of vitals, default ranges, percentages, icon set, limits and the Home grid live in **one configuration file**, `Server/config/vitals.json`. The apps receive them from the server, so changing them needs no app update. |
| V7 | A doctor can **add a vital for one patient**: a name, an icon from the predefined set, a unit and its range. |
| V8 | **Urine** has two values, water intake and urine output, and is coloured by output as a percentage of intake: 85 % or more green, from 70 % yellow, less red. Both numbers are in the configuration file. |
| V9 | A reading's **time** is chosen when it is entered: now, or up to 12 hours back (configurable). There is no way to edit a reading or its time afterwards. |
| V10 | Everything is **forward and backward compatible** by the rules in section 7. |

## 3. Screens

- **Home, user** (every person: patients and also hospital and platform administrators, who are
  people with their own health too; not doctors, assistants, labs or hospital accounts): the
  search section is gone (Search is in the bottom bar). In its place is a
  block of two rows of five tiles: Water, BP, Sugar, Heart rate, Urine, Weight, Temperature,
  Oxygen, Sleep and **More**.
  - Each tile shows the vital's icon in the colour of the latest reading (grey if none) and
    that value.
  - Tapping a tile opens the entry sheet: the value or values, the time (Now, or Earlier
    within 12 hours), an optional note, and the last five entries.
  - **More** opens the full vitals page, where vitals a doctor added also appear.
- **Home, doctor:** one row the height of the bottom bar's icons with three icons: Red,
  Yellow and Green, each with its unread count. It fills in live as patients record readings.
- **Patient alerts (doctor):** the unread alerts of one colour, sortable by newest, patient
  or vital.
  - Swipe **left**: removes the alert from the active list (for this doctor only).
  - Swipe **right**: opens the patient's profile.
  - A tick button and a tap do the same without swiping.
- **Vitals page:** every vital with its latest value, then the history, newest entry first,
  with a filter per vital.
  - The person taps a vital to enter a value.
  - Their doctor reaches it from the patient's page (**Vitals** in the bottom bar) and
    instead sees each range, can edit it, and can add a vital for the patient.
- **Bottom bar:** reduced to the height of an icon with its label.

## 4. Data structures

Five Cassandra tables (`Server/db/schema.cql`). Every table has `schema_version` and an
`extra map<text,text>` so later versions can add data without a migration.

| Table | Key | Holds |
| --- | --- | --- |
| `vital_readings` | `(user_id), reading_id DESC` | One person's readings, newest entry first: `code`, `vals` (component → number), `taken_at`, `note`, `entered_by`, `severity`. |
| `vital_readings_by_type` | `(user_id), code, reading_id DESC` | The same rows per vital: history of one vital and its latest value. |
| `vital_types_by_user` | `(user_id), code` | Vitals a doctor added for one patient: label, icon, unit, components with range, margin. |
| `vital_ranges` | `(user_id), code` | A doctor's range for one patient and vital; replaces the default. |
| `vital_alerts_by_doctor` | `(doctor_id), alert_id DESC` | Each doctor's inbox: patient, vital, values, severity, `is_read`. Expires after 90 days. |

- A reading stores **whose it is** (`user_id`), **date** (`taken_at`, plus the entry time in
  `reading_id`), **value** (`vals`) and its colour. **Read / unread** is stored per doctor in
  the inbox.
- Sorting: by entry (default), by vital (`vital_readings_by_type`), and in the doctor's inbox
  by time, patient or vital.
- Built-in vitals are not in the database; they come from the configuration file.

## 5. API

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/v1/vitals/config` | Icons, margin bounds, Home grid, back-dating limit. |
| GET | `/v1/vitals/types?user=` | The person's vitals. Ranges and thresholds are included only for their doctor. |
| POST | `/v1/vitals/types` | Doctor: add a vital for a patient (`user_id`, `label`, `icon`, `unit`, `low`, `high`, `margin_pct`). |
| GET | `/v1/vitals/latest?user=` | Newest reading of each vital. |
| GET | `/v1/vitals/readings?user=&code=&limit=` | Newest entry first. |
| POST | `/v1/vitals/readings` | The caller's own reading: `code`, `values`, `taken_at?`, `note?`. Returns it with its colour. |
| PUT | `/v1/vitals/ranges/{user}/{code}` | Doctor: `{ components: { key: { low?, high? } }, margin_pct? }`. |
| GET | `/v1/vitals/alerts?severity=&sort=time\|user\|vital` | Doctor: unread alerts and `counts { red, yellow, green }`. |
| POST | `/v1/vitals/alerts/{id}/read` | Removes the alert from the caller's active list. |

- **Who may do what:** a person reads and records their own vitals. A doctor reads those of a
  patient in their contacts and is the only one who sets ranges or adds vitals. Nobody else
  has access (labs, hospitals, assistants and administrators included).
- **Live update:** the server sends `{"type":"vital.alert","patient_id","severity","code"}`
  over the WebSocket to the patient's doctors.
- **No edit or delete** endpoint exists for readings.

## 6. Configuration file

`Server/config/vitals.json` is compiled into the server as its defaults. On the LAN trial the
compose file mounts the folder and sets `VITALS_CONFIG`, so after editing the file a
`docker compose restart care-api` is enough. A file that fails the checks stops the server at
start-up with the reason.

| Section | Settings |
| --- | --- |
| `colours` | `yellow_margin_pct` (30), `margin_pct_min` / `margin_pct_max` (what a doctor may set) |
| `home` | `columns` (5), `rows` (2) of the vitals block |
| `limits` | `value_max`, `note_max_chars`, `custom_label_max_chars`, `custom_unit_max_chars`, `readings_default_limit`, `readings_max_limit`, `doctor_inbox_window`, `backdate_hours` (12) |
| `icons` | The predefined icon keys a doctor can choose from |
| `vitals` | Each built-in vital: `code`, `label`, `icon`, `unit`, `components` with default `low` / `high`, optional `margin_pct`, optional `ratio` |
| `vitals[].ratio` | `numerator`, `denominator`, `green_min_pct` (85), `yellow_min_pct` (70) — used by Urine |

What a doctor sets for one patient (range, margin, added vitals) is stored in the database and
wins over this file. The default ranges are general adult starting points for a trial, not
clinical advice.

## 7. Compatibility rules (vitals and DataItem)

These apply to every structure the apps and server exchange, DataItem included.

1. **Add, never change.** New information goes in new, optional fields. Existing fields keep
   their name, type and meaning.
2. **Readers ignore what they don't know.** The server ignores unknown request fields. The
   apps ignore unknown response fields (`ignoreUnknownKeys` on Android, `Codable` on iOS).
3. **Missing fields have defaults.** Anything an older peer might not send is optional or has
   a default, on both sides.
4. **No new values in existing closed lists.** A new kind of thing gets a new field or a new
   endpoint, not a new value in an existing enum. Vitals carry colour and icon as plain text;
   an unknown colour reads as green and an unknown icon as "other".
5. **`schema_version`** is stored with every vitals row and sent in every vitals object. It
   changes only if the meaning of an existing field must change.
6. **Database:** tables are created with `IF NOT EXISTS`, columns added with
   `ALTER TABLE … ADD IF NOT EXISTS`, and each vitals table has an `extra` map for small
   additions. `schema.cql` can be re-run on an existing database.
7. **Configuration:** sections added later are optional in the file, so a file written for an
   older server still loads.
8. **Older server, newer app:** if the vitals endpoints are missing, Home simply leaves the
   vitals section out.

Known limit: DataItem's existing enums (`primary_kind`, `status`, attachment kind, role) are
still closed lists in the apps. Android now falls back to a field's default when it has one;
fields without a default would still fail on a value added later, so rule 4 must be followed.

## 8. Tests

### 8.1 Server — `cargo test --lib` (Server/care-api)

| Id | Test (`vitals.rs`) | Checks |
| --- | --- | --- |
| V1 | `v1_inside_range_is_green_including_the_ends` | Green, both ends included |
| V2 | `v2_up_to_30_percent_outside_is_yellow` | Yellow up to exactly 30 % |
| V3 | `v3_beyond_the_margin_is_red` | Red beyond 30 % |
| V4 | `v4_the_doctor_can_change_the_margin` | Margin 10, 50 and 0 |
| V5 | `v5_blood_pressure_takes_the_worse_of_its_two_values` | Multi-value vitals |
| V6 | `v6_open_ranges_and_no_range` | One-sided and absent ranges |
| V7 | `v7_a_persons_own_range_replaces_the_default` | 150 is green for 80–160, yellow for 80–120 |
| V8 | `v8_readings_and_ranges_are_validated` | Missing, negative, unknown values; bad ranges |
| V9 | `v9_only_a_connected_doctor_reads_or_configures_someone_elses_vitals` | Access rules |
| V10 | `v10_catalogue_is_well_formed_and_tolerant_of_newer_data` | Catalogue, unknown fields and colours |
| V11 | `v11_configuration_file_sets_colours_and_defaults` | Editing the file changes behaviour |
| V12 | `v12_a_broken_configuration_is_refused_with_the_reason` | Bad files; an older file without `home` loads |
| V13 | `v13_urine_is_coloured_by_output_as_a_percentage_of_intake` | 85 / 70 thresholds from the file; zero intake refused |
| V14 | `v14_time_is_now_or_within_the_last_12_hours` | Back-dating window |

**Result, 2 Oct 2026:** 41 passed, 0 failed (V1–V14 plus the existing 27).
`cargo build --bins` succeeded.

**Schema, 2 Oct 2026:** `schema.cql` applied twice to a fresh Cassandra 5.0 with no errors;
a reading was inserted into `vital_readings` and read back.

### 8.2 Android — `gradlew :app:testDebugUnitTest` (WUI/android)

| Id | Test | Checks |
| --- | --- | --- |
| C1 | `c1_a_newer_servers_extra_fields_are_ignored` | Forward compatibility |
| C2 | `c2_an_unknown_colour_reads_as_green_instead_of_failing` | Unknown colour and missing fields |
| C3 | `c3_an_older_servers_missing_fields_get_defaults` | Backward compatibility |
| C4 | `c4_values_are_shown_in_the_vitals_own_order` | "120/80 mmHg" formatting |
| C5 | `c5_time_can_be_set_only_within_the_last_12_hours` | Entry time rule |
| C6 | `c6_urine_is_two_values_with_a_percentage` | Urine percentage; thresholds not sent to the patient |
| B8 | `b8_vitals_is_for_a_doctor_on_a_patients_page` | Vitals operation only for a doctor on a patient's page |

**Result, 2 Oct 2026:** 28 passed, 0 failed for the whole app (these 7, the 6 offline-cache tests
in `OfflineCache_Design.md`, and the existing 15). The app compiles.

### 8.3 On a device

- **Checked, 2 Oct 2026 (Android emulator, demo data, as a user):** Home shows the Vitals
  label, two rows of five tiles with More last, Open items below and no search section; the
  bottom bar is the reduced height.
- **Not checked yet:** entering a reading and its colour, the time picker, the Urine sheet,
  the doctor's alert row and list with both swipes, range editing, adding a vital, and
  everything against the real server. Steps 34–45 in `TestingReadMe.txt` cover these.

### 8.4 iOS

Written to mirror Android (`Models/VitalModels.swift`, `Features/Vitals.swift`, Home, repository,
routes). **Not compiled**: no Mac was available. Build it and repeat 8.3 before relying on it.
