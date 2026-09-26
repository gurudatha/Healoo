// Build tool versions (Healoo WUI 0.3).
// Gradle 9.3.0 + Android Gradle plugin 9.0.1 + JDK 17.
//   - AGP 9.0.x needs Gradle 9.1.0 or newer and JDK 17 or newer.
//   - AGP 9.1 needs Gradle 9.3.1, so stay on 9.0.x while using Gradle 9.3.0.
//   - AGP 9 compiles Kotlin itself (built-in Kotlin); the org.jetbrains.kotlin.android
//     plugin is no longer applied. The Compose and serialization compiler plugins must
//     match AGP's Kotlin version (2.2.10 for AGP 9.0).
plugins {
    id("com.android.application") version "9.0.1" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.10" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.2.10" apply false
    id("com.google.gms.google-services") version "4.4.4" apply false
}
