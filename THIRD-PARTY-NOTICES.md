# Third-party notices

Healoo itself is licensed under the GNU General Public License, version 3 or later; see
[`COPYRIGHT.md`](COPYRIGHT.md) and [`LICENSE`](LICENSE). This file lists the third-party
material Healoo includes or is built with, each under its own licence.

Licence texts are in:

- [`LICENSES/Apache-2.0.txt`](LICENSES/Apache-2.0.txt) — Apache License 2.0
- [`LICENSES/OFL-1.1.txt`](LICENSES/OFL-1.1.txt) — SIL Open Font License 1.1
- [`Server/THIRD-PARTY-LICENSES.txt`](Server/THIRD-PARTY-LICENSES.txt) — every Rust library of
  the server with its own licence, copyright and notice files

How each section was produced is stated in it. Sections marked "from package metadata" were
read from the packages themselves on 6 October 2026; update them when dependencies change.

## 1. Files included in this repository

| What | Where | Copyright | Licence |
| --- | --- | --- | --- |
| Figtree font (Regular, Medium, SemiBold), version 2.002 | `WUI/android/app/src/main/res/font/figtree_*.ttf` | Copyright 2022 The Figtree Project Authors (https://github.com/erikdkennedy/figtree) | SIL Open Font License 1.1 |
| Fraunces font (SemiBold), version 1.000 | `WUI/android/app/src/main/res/font/fraunces_semibold.ttf` | Copyright 2020 The Fraunces Project Authors (github.com/undercasetype/Fraunces) | SIL Open Font License 1.1 |
| Material icons (24 icon definitions) | `WUI/android/app/src/main/java/com/healoo/app/ui/icons/` | Copyright 2025 The Android Open Source Project | Apache-2.0 (header kept in each file) |

The font copyright lines are the ones recorded inside the font files. The fonts are
unmodified. The iOS app expects the same fonts in `WUI/ios/Healoo/Resources/Fonts/`; they are
not committed there.

## 2. Server (`Server/care-api`) — Rust libraries

From package metadata. 475 libraries, all under permissive licences (MIT, Apache-2.0, BSD,
ISC, Zlib, Unicode-3.0, Unlicense, 0BSD, CC0, BSL-1.0, CDLA-Permissive-2.0). Where a library
offers a choice, Healoo uses it under the MIT licence or the first permissive one listed.

The full list, with each library's own licence, copyright and notice files, is
[`Server/THIRD-PARTY-LICENSES.txt`](Server/THIRD-PARTY-LICENSES.txt). Regenerate it with
`Server/tools/third_party_licenses.py` after changing `Cargo.toml`.

Bundled inside those libraries and therefore inside the server binaries:

- The `hayro` PDF renderer carries substitute fonts for the standard PDF fonts (Foxit fonts,
  BSD-3-Clause) and ICC colour profiles; their notices are in the file above
  (`hayro-interpret`, `assets/LICENSE_FOXIT` and `assets/CGATS_LICENSE.txt`).
- `rdkafka` compiles in librdkafka (BSD-2-Clause) and the libraries it bundles; their
  notices are in the file above (`rdkafka-sys`).

## 3. Server Docker image

The image is built on Debian (`debian:trixie-slim`) and adds the packages `ca-certificates`,
`libsasl2-2`, `libcurl4` and `curl`. Debian's own packages, including the GNU C Library
(LGPL-2.1-or-later), keep their notices inside the image under `/usr/share/doc/*/copyright`.
The server links to them dynamically.

## 4. Android app (`WUI/android`) — libraries

From package metadata: the 169 artifacts on the release runtime classpath, with the licence
each one states. 156 are Apache-2.0, 1 is MIT and 12 are Google libraries under Google's own
terms, which are not open-source licences. `COPYRIGHT.md` grants the additional permission
needed to combine Healoo with those 12.

Not reproduced here: any `NOTICE` files inside the Apache-2.0 artifacts. When the app is
distributed, they should be shown together with this list (for example on an "Open-source
licences" screen in the app).

MIT licence for `com.auth0.android:auth0` (Copyright (c) Auth0, Inc.):

> Permission is hereby granted, free of charge, to any person obtaining a copy of this
> software and associated documentation files (the "Software"), to deal in the Software
> without restriction, including without limitation the rights to use, copy, modify, merge,
> publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
> to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
> INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
> PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
> FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
> OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
> DEALINGS IN THE SOFTWARE.

| Artifact | Version | Licence stated by the package |
| --- | --- | --- |
| `androidx.activity:activity` | 1.10.1 | Apache-2.0 |
| `androidx.activity:activity-compose` | 1.10.1 | Apache-2.0 |
| `androidx.activity:activity-ktx` | 1.10.1 | Apache-2.0 |
| `androidx.annotation:annotation` | 1.9.1 | Apache-2.0 |
| `androidx.annotation:annotation-experimental` | 1.4.1 | Apache-2.0 |
| `androidx.annotation:annotation-jvm` | 1.9.1 | Apache-2.0 |
| `androidx.appcompat:appcompat` | 1.6.1 | Apache-2.0 |
| `androidx.appcompat:appcompat-resources` | 1.6.1 | Apache-2.0 |
| `androidx.arch.core:core-common` | 2.2.0 | Apache-2.0 |
| `androidx.arch.core:core-runtime` | 2.2.0 | Apache-2.0 |
| `androidx.autofill:autofill` | 1.0.0 | Apache-2.0 |
| `androidx.browser:browser` | 1.4.0 | Apache-2.0 |
| `androidx.collection:collection` | 1.5.0 | Apache-2.0 |
| `androidx.collection:collection-jvm` | 1.5.0 | Apache-2.0 |
| `androidx.collection:collection-ktx` | 1.5.0 | Apache-2.0 |
| `androidx.compose:compose-bom` | 2025.08.01 | Apache-2.0 |
| `androidx.compose.animation:animation` | 1.9.0 | Apache-2.0 |
| `androidx.compose.animation:animation-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.animation:animation-core` | 1.9.0 | Apache-2.0 |
| `androidx.compose.animation:animation-core-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.foundation:foundation` | 1.9.0 | Apache-2.0 |
| `androidx.compose.foundation:foundation-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.foundation:foundation-layout` | 1.9.0 | Apache-2.0 |
| `androidx.compose.foundation:foundation-layout-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.material:material-icons-core` | 1.7.8 | Apache-2.0 |
| `androidx.compose.material:material-icons-core-android` | 1.7.8 | Apache-2.0 |
| `androidx.compose.material:material-ripple` | 1.9.0 | Apache-2.0 |
| `androidx.compose.material:material-ripple-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.material3:material3` | 1.3.2 | Apache-2.0 |
| `androidx.compose.material3:material3-android` | 1.3.2 | Apache-2.0 |
| `androidx.compose.runtime:runtime` | 1.9.0 | Apache-2.0 |
| `androidx.compose.runtime:runtime-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.runtime:runtime-annotation` | 1.9.0 | Apache-2.0 |
| `androidx.compose.runtime:runtime-annotation-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.runtime:runtime-saveable` | 1.9.0 | Apache-2.0 |
| `androidx.compose.runtime:runtime-saveable-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-geometry` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-geometry-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-graphics` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-graphics-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-text` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-text-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-tooling-preview` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-tooling-preview-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-unit` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-unit-android` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-util` | 1.9.0 | Apache-2.0 |
| `androidx.compose.ui:ui-util-android` | 1.9.0 | Apache-2.0 |
| `androidx.concurrent:concurrent-futures` | 1.1.0 | Apache-2.0 |
| `androidx.core:core` | 1.15.0 | Apache-2.0 |
| `androidx.core:core-ktx` | 1.15.0 | Apache-2.0 |
| `androidx.core:core-viewtree` | 1.0.0 | Apache-2.0 |
| `androidx.cursoradapter:cursoradapter` | 1.0.0 | Apache-2.0 |
| `androidx.customview:customview` | 1.0.0 | Apache-2.0 |
| `androidx.customview:customview-poolingcontainer` | 1.0.0 | Apache-2.0 |
| `androidx.documentfile:documentfile` | 1.0.0 | Apache-2.0 |
| `androidx.drawerlayout:drawerlayout` | 1.0.0 | Apache-2.0 |
| `androidx.emoji2:emoji2` | 1.4.0 | Apache-2.0 |
| `androidx.emoji2:emoji2-views-helper` | 1.4.0 | Apache-2.0 |
| `androidx.exifinterface:exifinterface` | 1.3.7 | Apache-2.0 |
| `androidx.fragment:fragment` | 1.3.6 | Apache-2.0 |
| `androidx.graphics:graphics-path` | 1.0.1 | Apache-2.0 |
| `androidx.interpolator:interpolator` | 1.0.0 | Apache-2.0 |
| `androidx.legacy:legacy-support-core-utils` | 1.0.0 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-common` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-common-java8` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-common-jvm` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-livedata` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-livedata-core` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-livedata-core-ktx` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-process` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-runtime` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-runtime-android` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-runtime-compose` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-runtime-compose-android` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-runtime-ktx` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-runtime-ktx-android` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel-android` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel-compose` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel-compose-android` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel-ktx` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel-savedstate` | 2.9.2 | Apache-2.0 |
| `androidx.lifecycle:lifecycle-viewmodel-savedstate-android` | 2.9.2 | Apache-2.0 |
| `androidx.loader:loader` | 1.0.0 | Apache-2.0 |
| `androidx.localbroadcastmanager:localbroadcastmanager` | 1.0.0 | Apache-2.0 |
| `androidx.navigation:navigation-common` | 2.9.3 | Apache-2.0 |
| `androidx.navigation:navigation-common-android` | 2.9.3 | Apache-2.0 |
| `androidx.navigation:navigation-compose` | 2.9.3 | Apache-2.0 |
| `androidx.navigation:navigation-compose-android` | 2.9.3 | Apache-2.0 |
| `androidx.navigation:navigation-runtime` | 2.9.3 | Apache-2.0 |
| `androidx.navigation:navigation-runtime-android` | 2.9.3 | Apache-2.0 |
| `androidx.print:print` | 1.0.0 | Apache-2.0 |
| `androidx.profileinstaller:profileinstaller` | 1.4.0 | Apache-2.0 |
| `androidx.resourceinspection:resourceinspection-annotation` | 1.0.1 | Apache-2.0 |
| `androidx.savedstate:savedstate` | 1.3.1 | Apache-2.0 |
| `androidx.savedstate:savedstate-android` | 1.3.1 | Apache-2.0 |
| `androidx.savedstate:savedstate-compose` | 1.3.1 | Apache-2.0 |
| `androidx.savedstate:savedstate-compose-android` | 1.3.1 | Apache-2.0 |
| `androidx.savedstate:savedstate-ktx` | 1.3.1 | Apache-2.0 |
| `androidx.startup:startup-runtime` | 1.1.1 | Apache-2.0 |
| `androidx.tracing:tracing` | 1.2.0 | Apache-2.0 |
| `androidx.vectordrawable:vectordrawable` | 1.1.0 | Apache-2.0 |
| `androidx.vectordrawable:vectordrawable-animated` | 1.1.0 | Apache-2.0 |
| `androidx.versionedparcelable:versionedparcelable` | 1.1.1 | Apache-2.0 |
| `androidx.viewpager:viewpager` | 1.0.0 | Apache-2.0 |
| `com.auth0.android:auth0` | 2.10.2 | MIT |
| `com.google.accompanist:accompanist-drawablepainter` | 0.32.0 | Apache-2.0 |
| `com.google.android.datatransport:transport-api` | 3.1.0 | Apache-2.0 |
| `com.google.android.datatransport:transport-backend-cct` | 3.1.9 | Apache-2.0 |
| `com.google.android.datatransport:transport-runtime` | 3.1.9 | Apache-2.0 |
| `com.google.android.gms:play-services-base` | 18.1.0 | Android Software Development Kit License |
| `com.google.android.gms:play-services-basement` | 18.3.0 | Android Software Development Kit License |
| `com.google.android.gms:play-services-cloud-messaging` | 17.2.0 | Android Software Development Kit License |
| `com.google.android.gms:play-services-code-scanner` | 16.1.0 | ML Kit Terms of Service |
| `com.google.android.gms:play-services-stats` | 17.0.2 | Android Software Development Kit License |
| `com.google.android.gms:play-services-tasks` | 18.1.0 | Android Software Development Kit License |
| `com.google.android.odml:image` | 1.0.0-beta1 | Android Software Development Kit License |
| `com.google.androidbrowserhelper:androidbrowserhelper` | 2.4.0 | Apache-2.0 |
| `com.google.code.gson:gson` | 2.8.9 | Apache-2.0 |
| `com.google.errorprone:error_prone_annotations` | 2.26.0 | Apache-2.0 |
| `com.google.firebase:firebase-annotations` | 16.2.0 | Apache-2.0 |
| `com.google.firebase:firebase-bom` | 33.3.0 | Apache-2.0 |
| `com.google.firebase:firebase-common` | 21.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-common-ktx` | 21.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-components` | 18.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-datatransport` | 18.2.0 | Apache-2.0 |
| `com.google.firebase:firebase-encoders` | 17.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-encoders-json` | 18.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-encoders-proto` | 16.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-iid-interop` | 17.1.0 | Android Software Development Kit License |
| `com.google.firebase:firebase-installations` | 18.0.0 | Apache-2.0 |
| `com.google.firebase:firebase-installations-interop` | 17.1.1 | Apache-2.0 |
| `com.google.firebase:firebase-measurement-connector` | 19.0.0 | Android Software Development Kit License |
| `com.google.firebase:firebase-messaging` | 24.0.1 | Apache-2.0 |
| `com.google.guava:listenablefuture` | 1.0 | Apache-2.0 |
| `com.google.mlkit:barcode-scanning-common` | 17.0.0 | ML Kit Terms of Service |
| `com.google.mlkit:common` | 18.9.0 | ML Kit Terms of Service |
| `com.google.mlkit:vision-common` | 17.0.0 | ML Kit Terms of Service |
| `com.google.zxing:core` | 3.5.3 | Apache-2.0 |
| `com.squareup.okhttp3:logging-interceptor` | 4.11.0 | Apache-2.0 |
| `com.squareup.okhttp3:okhttp` | 4.12.0 | Apache-2.0 |
| `com.squareup.okio:okio` | 3.9.0 | Apache-2.0 |
| `com.squareup.okio:okio-jvm` | 3.9.0 | Apache-2.0 |
| `com.squareup.retrofit2:converter-kotlinx-serialization` | 2.11.0 | Apache-2.0 |
| `com.squareup.retrofit2:retrofit` | 2.11.0 | Apache-2.0 |
| `io.coil-kt:coil` | 2.7.0 | Apache-2.0 |
| `io.coil-kt:coil-base` | 2.7.0 | Apache-2.0 |
| `io.coil-kt:coil-compose` | 2.7.0 | Apache-2.0 |
| `io.coil-kt:coil-compose-base` | 2.7.0 | Apache-2.0 |
| `javax.inject:javax.inject` | 1 | Apache-2.0 |
| `org.jetbrains:annotations` | 23.0.0 | Apache-2.0 |
| `org.jetbrains.kotlin:kotlin-stdlib` | 2.2.10 | Apache-2.0 |
| `org.jetbrains.kotlin:kotlin-stdlib-common` | 2.2.10 | Apache-2.0 |
| `org.jetbrains.kotlin:kotlin-stdlib-jdk7` | 1.9.0 | Apache-2.0 |
| `org.jetbrains.kotlin:kotlin-stdlib-jdk8` | 1.9.0 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-android` | 1.8.1 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-bom` | 1.8.1 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-core` | 1.8.1 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm` | 1.8.1 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-coroutines-play-services` | 1.8.1 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-serialization-bom` | 1.9.0 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-serialization-core` | 1.9.0 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-serialization-core-jvm` | 1.9.0 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-serialization-json` | 1.9.0 | Apache-2.0 |
| `org.jetbrains.kotlinx:kotlinx-serialization-json-jvm` | 1.9.0 | Apache-2.0 |
| `org.jspecify:jspecify` | 1.0.0 | Apache-2.0 |

JUnit 4.13.2 (Eclipse Public License 1.0) is used only to run the unit tests and is not part
of the app.

## 5. iOS app (`WUI/ios`) — libraries

**Not from package metadata.** The iOS packages are resolved by Swift Package Manager on a
Mac and were not available when this file was written, so this section lists only the two
direct dependencies named in `project.yml`, with the licences their projects publish.
Complete it from `Package.resolved` after the first build on a Mac.

| Package | Licence |
| --- | --- |
| Auth0.swift (https://github.com/auth0/Auth0.swift), with its dependencies JWTDecode.swift and SimpleKeychain | MIT |
| Firebase SDK for Apple platforms (https://github.com/firebase/firebase-ios-sdk), product FirebaseMessaging, with the Google libraries it depends on | Apache-2.0 for the SDK; some dependencies have their own licences |

## 6. Software Healoo runs beside, but does not include

These are separate programs started from the Docker Compose files. They are not part of
Healoo and are not distributed with it; each is downloaded from its own publisher.

| Program | Used for | Licence |
| --- | --- | --- |
| Apache Cassandra 5.0 | Database | Apache-2.0 |
| Apache Kafka 3.8 | Events | Apache-2.0 |
| Kafka UI (`provectuslabs/kafka-ui`) | Viewing events on the LAN trial | Apache-2.0 |
| SeaweedFS 4.48 | File storage in the internet deployment | Apache-2.0 |
| Caddy 2 | HTTPS in the internet deployment | Apache-2.0 |

The licences in this section are the ones those projects publish; they were not read from
the downloaded images.
