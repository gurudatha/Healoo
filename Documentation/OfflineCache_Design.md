# Offline message cache — design note

Status: **implemented** on Android (built and unit-tested). iOS is written but not compiled.
Not yet tried on a device with the server switched off.

## What it does

The app keeps a copy of what the Messages screens need, so discussions can be read when the
server can't be reached:

| Kept | When it is saved |
| --- | --- |
| The signed-in user's profile | Every time it is loaded |
| The conversation list (Messages tab) | Every time it is loaded |
| An item's header | When its discussion or the item is opened |
| An item's messages | When its discussion is opened, and after sending a message |

- **Online:** the app always loads from the server and refreshes the copy.
- **Offline (no connection):** the app shows the saved copy. Discussions that were never
  opened on this phone have no copy and show the usual error.
- **Sending** needs the server; nothing is queued.
- **Sign-out** deletes the whole cache, so the next account can't read it.
- The copy lives in the app's private storage. The server stays the source of truth.

## Size limit

- At most **40 MB** in total (`message_cache.max_mb`).
- When a save goes over the limit, the files with the **oldest dates** are removed until about
  **2 MB** is free again (`message_cache.trim_mb`). A discussion's date is the time of its
  newest message. The discussion just saved is never the one removed.
- This is deliberately simple, not efficient: each discussion is one JSON file.

## Configuration

`WUI/app-config.json`, bundled into both apps at build time (rebuild after changing it):

| Setting | Default | Meaning |
| --- | --- | --- |
| `message_cache.max_mb` | 40 | Total size of the offline copy |
| `message_cache.trim_mb` | 2 | How much of the oldest data is removed when full |
| `file_cache.max_files` | 15 | Full images and PDFs kept for the viewer |
| `file_cache.max_mb` | 90 | Their total size |

A missing setting, or a missing file, falls back to these defaults.

## Limits to know

- Opening the app with no connection works only if the saved sign-in is still valid. An
  expired Auth0 session needs the network to refresh.
- Home, Search, Upload and vitals still need the server.
- Messages that arrive while a discussion is closed are added to the copy the next time the
  discussion is opened online.

## Tests — `gradlew :app:testDebugUnitTest` (`OfflineCacheTest.kt`)

| Id | Test | Checks |
| --- | --- | --- |
| O1 | `o1_what_is_saved_can_be_read_back_without_a_server` | Save and read back; nothing for unopened discussions |
| O2 | `o2_stays_under_the_limit` | Never above the limit after many saves |
| O3 | `o3_when_full_the_oldest_by_date_go_first_and_about_trim_mb_is_freed` | Oldest dates removed, not oldest written; trim size |
| O4 | `o4_the_discussion_just_saved_is_never_the_one_removed` | The current discussion survives |
| O5 | `o5_a_discussion_is_dated_by_its_newest_message` | Dating rule |
| O6 | `o6_limits_come_from_app_config_with_defaults` | The shipped file says 40 / 2; partial files; sign-out clears |

**Result, 2 Oct 2026:** 6 passed, 0 failed. The tests use small limits (kilobytes) to exercise
the same rule. **Not tested:** the real 40 MB limit on a phone, and reading discussions with
the server stopped (step 46 in `TestingReadMe.txt`).
