HEALOO WUI 0.3 - RUNNING AND TESTING FROM VISUAL STUDIO CODE
=============================================================

VS Code can edit and run both apps, but it only drives the command-line tools.
Android works on Windows, macOS or Linux. iOS needs a Mac with Xcode installed,
even if you never open Xcode itself.


1. ONE-TIME SETUP (ANDROID)
---------------------------

Install:
  - JDK 17 (for example Eclipse Temurin 17). Gradle 9 will not start on an
    older JDK such as 11, so JDK 17 must be the one Gradle uses (see below).
  - Android SDK command-line tools (developer.android.com -> "Command line tools only");
    unzip them to ~/Android/Sdk/cmdline-tools/latest
  - Gradle 9.3.0, used only once to create the wrapper
    (the project uses Android Gradle plugin 9.0.1, which needs Gradle 9.1 or newer)
  - VS Code extensions: "Kotlin" (fwcd) for syntax highlighting,
    and optionally "Gradle for Java"

Set environment variables.
  macOS / Linux - add to ~/.zshrc or ~/.bashrc:

    export ANDROID_HOME=$HOME/Android/Sdk
    export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator

  Windows - set ANDROID_HOME in System -> Environment Variables and add the
  same three folders to Path.

Make JDK 17 the Java that Gradle uses. Check first:

    java -version            (must say 17 or newer)
    echo %JAVA_HOME%         (Windows cmd)   /   echo $JAVA_HOME   (macOS/Linux)

  If either shows 11 (or another older version):
  - Windows: System -> Environment Variables -> set JAVA_HOME to the JDK 17
    folder, e.g. C:\Program Files\Eclipse Adoptium\jdk-17.0.16.8-hotspot, and
    in Path move %JAVA_HOME%\bin above any other Java entry (or delete the
    JDK 11 entry). Close and reopen VS Code and terminals afterwards.
  - macOS:  export JAVA_HOME=$(/usr/libexec/java_home -v 17)   in ~/.zshrc
  - Linux:  sudo update-alternatives --config java   (pick 17) and set JAVA_HOME
  - To keep JDK 11 as the default for other work, leave JAVA_HOME alone and
    instead add this line to ~/.gradle/gradle.properties (your user folder):
        org.gradle.java.home=C:/Program Files/Eclipse Adoptium/jdk-17.0.16.8-hotspot
  - VS Code: if the Gradle extension still reports JDK 11, set
    "java.jdt.ls.java.home" and "java.import.gradle.java.home" to the JDK 17
    folder in settings.json.

Install the SDK parts and create an emulator. Use a Play Store image, because
the QR scanner needs Google Play services:

    sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "emulator" "system-images;android-35;google_apis_playstore;x86_64"
    avdmanager create avd -n healoo -k "system-images;android-35;google_apis_playstore;x86_64"

  On an Apple Silicon Mac, use arm64-v8a instead of x86_64 in both lines.
  (The app compiles against SDK 36 but targets and runs on Android 15 / API 35,
   so the API 35 emulator image is still right.)

Prepare the project. Unzip healoo-wui-0.3.zip and open the healoo-wui folder
in VS Code (File -> Open Folder). Then, in the VS Code terminal:

    cd android
    gradle wrapper --gradle-version 9.3.0        (creates ./gradlew - only once)
    ./gradlew --version                          (check: "Launcher JVM" and "Daemon JVM" show 17)

  Already made a wrapper with Gradle 8.9 earlier? Run
    ./gradlew wrapper --gradle-version 9.3.0
  once to move it to 9.3.0.
    echo "sdk.dir=$ANDROID_HOME" > local.properties

  On Windows, write the path with forward slashes, for example:
    sdk.dir=C:/Users/you/Android/Sdk

Add the four font files to android/app/src/main/res/font/:
    fraunces_semibold.ttf, figtree_regular.ttf, figtree_medium.ttf, figtree_semibold.ttf
  (download Fraunces and Figtree from fonts.google.com, use the files in /static,
   rename to these lower-case names). The build fails without them.


2. RUN THE ANDROID APP
----------------------

Start the emulator in one terminal and leave it running:

    emulator -avd healoo

Build, install and launch in a second terminal:

    cd android
    ./gradlew installDebug                       (Windows: gradlew.bat installDebug)
    adb shell am start -n com.healoo.app/.MainActivity

Watch the logs, including crashes:

    adb logcat --pid=$(adb shell pidof com.healoo.app)

  On Windows, "adb logcat *:E" is simpler.

Run it with one key press. Save the following as .vscode/tasks.json in the
healoo-wui folder. Then Ctrl/Cmd+Shift+B builds, installs and launches the app:

{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Healoo: run Android",
      "type": "shell",
      "command": "./gradlew installDebug && adb shell am start -n com.healoo.app/.MainActivity",
      "windows": { "command": ".\\gradlew.bat installDebug; adb shell am start -n com.healoo.app/.MainActivity" },
      "options": { "cwd": "${workspaceFolder}/android" },
      "group": { "kind": "build", "isDefault": true },
      "problemMatcher": []
    }
  ]
}


3. RUN THE iOS APP (MAC ONLY)
-----------------------------

One-time setup:
  - Install Xcode from the App Store and open it once to accept the licence
  - Run: brew install xcodegen
  - Add the four font files to ios/Healoo/Resources/Fonts/:
      Fraunces-SemiBold.ttf, Figtree-Regular.ttf, Figtree-Medium.ttf, Figtree-SemiBold.ttf

Build and run on the simulator:

    cd ios
    xcodegen generate
    xcodebuild -project Healoo.xcodeproj -scheme Healoo -destination 'platform=iOS Simulator,name=iPhone 15' -derivedDataPath build CODE_SIGNING_ALLOWED=NO build
    open -a Simulator
    xcrun simctl boot "iPhone 15" 2>/dev/null
    xcrun simctl install booted build/Build/Products/Debug-iphonesimulator/Healoo.app
    xcrun simctl launch booted com.healoo.app

Notes:
  - The first build downloads the Auth0 and Firebase packages, so it needs
    internet and takes a few minutes.
  - If your Xcode doesn't include an "iPhone 15" simulator, run
    "xcrun simctl list devices" and use a name from that list.
  - The SweetPad extension adds build and run buttons for iOS inside VS Code,
    if you'd rather not type the commands.


4. TEST THE FUNCTIONALITY
-------------------------

Both apps start in demo mode, so no backend is needed. Go through these checks
in order:

  1. Demo sign-in: choose Lakshmi K. (patient). Home shows counters and open items
     (report, discussion, appointment and alert items).
  2. Viewer: open "CBC - Complete blood count", then tap an attachment. Swipe
     through all 4 files, scroll the PDF pages, pinch or double-tap to zoom,
     and tap the thumbnails.
  3. Share and revoke: in the same item, use "Share with..." to add a contact,
     then "Revoke" it.
  4. Active sharing: go to Settings -> Active sharing. Revoke one item, then
     try "Stop all".
  5. Notification settings: turn a toggle off, go to another tab and come back.
     It should still be off.
  6. Edit profile: change your name and save. Your QR code is shown on this screen.
  7. Live messages: open Messages. Each row is one item's discussion with a
     person. Open "Latest BP readings" and send a message. A reply should appear
     about 1.5 seconds later without refreshing.
  8. Search: search for HL-3K8M1, then tap "Add". The result should now show as
     connected.
  9. New report: in the Upload tab keep "Report", attach several images and PDFs,
     reorder them, upload, and check the new item opens.
 10. Lab flow: Settings -> Log out, then sign in as City Diagnostics. Open
     Lakshmi's page and tap Upload in the bottom bar. Lakshmi is locked as the
     patient; "Share with" offers doctors only.
 11. Patient sees the lab upload: log out, sign in as Lakshmi, and check the
     lab's upload is on Home.
 12. Doctor view: sign in as Dr. Anitha Rao. Items shared with her or with her
     hospital appear. On an item, "Discussion" opens its messages.
 13. Item as a container: open "CBC - Complete blood count". Check the sections:
     attachments, discussion (1 new), a bi-weekly appointment for 3 months with
     its visit count, and a daily medication alert.
 14. Book an appointment: on the CBC item tap "Book". Choose Monthly for 1 month:
     the sheet explains it gives only one visit and won't book. Choose Weekly for
     2 months: it shows 9 visits. Book it.
 15. One visit: on a visit, use the menu to "Move this visit" to another day,
     then cancel a different visit. The rest of the series stays the same.
 16. Alerts: "Add alert" on any item; delete it again with the bin icon.
 17. Add files: on the CBC item tap "Add files", pick a PDF and add it.
 18. New appointment item: Upload tab -> "Appointment", pick Dr. Anitha Rao,
     date, time, no repeat. The new item opens with the appointment.
 19. Close with rating: on "Latest BP readings" tap the Open status, give 4
     stars and feedback, close. The item shows the closure; messaging is off.
     Reopen it with the Reopen button.
 20. Rating is patient-only: sign in as Dr. Anitha Rao, close an item: the
     sheet has feedback but no stars.
 21. Start a discussion: open Dr. Srinivas Rao's page (Search -> HL-3K8M1),
     tap Message in the bottom bar, write a message, Send. A new discussion
     item opens.

Page operations (Documentation/PageOperations_Design.md, section 7.3). Record
the result of each step in that section.

 22. Global bar: on Home the bar shows Home, Search, Upload, Messages and a
     three-dot More button; More lists Settings.
 23. User page: as Dr. Anitha Rao open Lakshmi's page. The bar shows Message,
     History, Share, Upload and no More button.
 24. Doctor page: as Lakshmi open Dr. Anitha Rao's page. The bar shows Message,
     History, Share, Book; More lists Upload.
 25. Not connected: as Lakshmi open Priya Rao's page (Search -> HL-4K7Q2). The
     bar shows only Add contact.
 26. Message with extra people: on Dr. Anitha Rao's page tap Message. She is a
     locked chip. Type "srin" in the search bar, filter Doctors, add Dr.
     Srinivas Rao, send. The discussion opens; both doctors can see it.
 27. Share a document: on Dr. Srinivas Rao's page tap Share, search "CBC",
     pick it, Share. The page's Shared items list now shows it.
 28. Referral: sign in as Dr. Anitha Rao, open Dr. Srinivas Rao's page (add
     him first) and tap Share. Pick the CBC item (Lakshmi owns it): the sheet
     says it can go to doctors only, and the search bar offers only doctors
     (typing "City" finds nothing). Share it. It works.
 29. Upload into an existing item: on Dr. Anitha Rao's page tap More ->
     Upload, choose "Existing item", pick the CBC item, add a PDF. The file is
     added to that item.
 30. Upload a new item: same screen with "New item": Dr. Anitha Rao is the
     locked "Share with" chip; the new item is shared with her.
 31. Book from a doctor page: tap Book on Dr. Anitha Rao's page. Upload opens
     in Appointment mode with her selected.
 32. Hospital page: open Test Hospital A (HL-1H0A1). The page lists Dr. Anitha
     Rao and Dr. Kavya Menon with a search bar; Doctors focuses it. Book ->
     pick Dr. Kavya Menon -> the appointment form opens for her at Hospital A.
 33. Hospital upload: sign in as Test Hospital A, open Lakshmi's page, Upload
     a report. Sign in as Lakshmi: the report is on Home.

Items that need extra setup:
  - QR scanning: on Android it needs the Play Store emulator image; the camera
    can be pointed at a virtual scene. On iOS it needs a real iPhone. To test
    it, display your own QR from Edit profile on a second device and scan it.
  - Push notifications: need google-services.json (Android, in android/app/) or
    GoogleService-Info.plist (iOS, in ios/Healoo/), and a real device for iOS.
    Without these files the apps run normally, just without push.
  - Auth0 sign-in: needs the settings from README section 5
    (healoo.useFakeData=false and the Auth0 values) and the backend running.


4b. TEST AGAINST THE TRIAL SERVER (DEVELOPER SIGN-IN)
-----------------------------------------------------

This uses the Rust backend (healoo-backend.zip) on your computer, with no Auth0.

 1. Start the backend's LAN profile (healoo-backend README section 3):
      cd healoo-backend/deploy/lan
      copy .env.lan.example to .env and set HOST_LAN_IP
      docker compose up -d --build
 2. Copy the certificate out of Docker:
      docker compose cp care-api:/data/tls/healoo-local-ca.crt .
    Android: nothing to install. Debug builds bundle it automatically from
    healoo-backend/deploy/lan (when healoo-backend sits next to healoo-wui) or
    from any .crt in android/certs/. The build output lists it.
    iOS Simulator: drag the file onto it, then Settings > General > About >
    Certificate Trust Settings > turn it on.
 3. Point the apps at the server:
      Android gradle.properties:  healoo.useFakeData=false
                                  healoo.apiBaseUrl=https://10.0.2.2:8443/
      iOS project.yml:            HealooUseFakeData: false
                                  HealooAPIBaseURL: https://localhost:8443/
                                  (then run xcodegen generate again)
    On a real phone use https://<HOST_LAN_IP>:8443/ instead.
 4. Rebuild and launch. The sign-in screen shows "Developer sign-in" with the
    server's test accounts. Tap Lakshmi K. (HL-2M9P4).
 5. Repeat checks 1-12 above. Differences from demo mode: data comes from the
    server, messages arrive live between two devices or emulators signed in
    as Lakshmi and Dr. Anitha Rao, and thumbnails/page counts appear a moment
    after an upload. Alerts fire as notifications (the scheduler checks every
    20 seconds; set an alert a couple of minutes ahead to test). After updating
    from v0.1, recreate and re-seed the database first:
      docker compose down -v
      docker compose up -d --build     (the care-seed job reloads the test data)
 6. Extra accounts for access-rule tests: Dr. Srinivas Rao (HL-3K8M1,
    independent doctor), Hospital A Admin (HL-8A2D4), Meena S. (HL-8S5T7,
    assistant to Dr. Rao). More test accounts (patients, doctors by
    designation, labs) are listed in Server/README.md.
 7. Administration: sign in as Hospital A Admin (HL-8A2D4), Settings ->
    Administration. Add a user and a doctor (designation chip), note the
    Healoo IDs. Delete (deactivate) the new doctor: it moves to the Deleted
    filter and can't sign in; Reactivate it. There is no way to delete a
    user, and DELETE /v1/admin/users/{id} answers 403.

If the account list shows "Can't reach the trial server", check the address,
that the certificate is installed, and that the backend is running
(https://<address>:8443/healthz should answer "ok" in a browser).


5. FIRST BUILD
--------------

The code hasn't been compiled yet, so the first "./gradlew installDebug" or
"xcodebuild" will probably report some errors, most likely around third-party
library versions (Auth0, Firebase, ML Kit). Copy the first few error lines and
send them back for fixing.
