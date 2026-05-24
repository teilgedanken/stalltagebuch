# 🥚 Wachtel-Stallbuch

Native Android-App zur Verwaltung von Wachteln und Eierproduktion, entwickelt mit Dioxus 0.7 und Rust.

## ✨ Features

- **Profilverwaltung:** Wachtel-Profile mit Name, Geschlecht, Geburtsdatum, Ringfarbe, Status und Fotos
- **Ereignis-Tracking:** Lebensereignisse pro Wachtel (Geboren, Krank, Gesund, Geschlachtet, etc.)
- **Foto-Verwaltung:** Mehrere Fotos pro Wachtel und Ereignis mit Galerie und Kamera-Integration
- **Eier-Tracking:** Tägliche Erfassung der Eierproduktion mit Historie
- **Statistiken:** Dashboard mit Durchschnittswerten und Zeitraum-Filtern
- **Native Android:** JNI-Integration für Kamera, Galerie und FileProvider

## 🏗️ Projektstruktur

```
stalltagebuch/
├── src/
│   ├── main.rs                      # Dioxus App Entry, Screen Routing
│   ├── error.rs                     # Zentrales Error-Handling
│   ├── camera.rs                    # JNI-Bridge für Camera & Gallery Intents
│   ├── filesystem.rs                # JNI-basierter Dateizugriff
│   ├── image_processing.rs          # Bild-Resize & Thumbnails (Placeholder)
│   ├── models/                      # Domain-Modelle (Wachtel, EggRecord)
│   ├── services/                    # Business Logic (Profile, Egg, Analytics)
│   └── components/                  # UI-Komponenten (Home, Profile, Tracking, Stats)
├── android/
│   ├── MainActivity.kt              # Custom Activity mit Camera/Gallery Intents
│   ├── AndroidManifest.xml          # Permissions & FileProvider Config
│   └── res/xml/file_paths.xml       # FileProvider Paths
├── assets/
│   ├── bulma.css                    # Bulma v1 UI-Framework (primäre Basis)
│   ├── main.css                     # App-spezifische Ergänzungen/Overrides
│   └── favicon.ico
├── build_android.sh                 # Wrapper Build-Script (siehe unten)
├── Cargo.toml                       # Rust Dependencies
└── Dioxus.toml                      # Dioxus CLI Config
```

## 🔧 Build & Entwicklung

### Voraussetzungen

- **Rust** (stable): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Dioxus CLI**: `curl -sSL http://dioxus.dev/install.sh | sh`
- **Android NDK & SDK** (API 28+, target 34)
- **Gradle** (automatisch via Android SDK)
- **adb** (Android Debug Bridge)

### Android-Target installieren

```bash
rustup target add x86_64-linux-android   # Emulator
rustup target add aarch64-linux-android  # Physisches Gerät
```

### Debug Build & Installation

**Wichtig:** Nutze das `build_android.sh` Wrapper-Script statt direktem `dx build`:

```bash
./build_android.sh
```

**Was das Script macht:**
1. Bereinigt alte Android-Build-Artefakte
2. Führt `dx build --platform android` aus
3. Kopiert custom `MainActivity.kt`, `AndroidManifest.xml`, `file_paths.xml`
4. Erstellt `BuildConfig.kt` Typealias (bridged `dev.dioxus.main` → `de.teilgedanken.stalltagebuch`)
5. Patched `build.gradle.kts` (Package-Name, SDK-Versionen)
6. Führt `gradlew assembleDebug` aus
7. Prüft ob MainActivity im APK enthalten ist

**APK-Pfad:**
```
target/dx/stalltagebuch/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

**Installation:**
```bash
adb install -r target/dx/stalltagebuch/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

**Logcat (Debugging):**
```bash
adb logcat | grep -iE "stalltagebuch|MainActivity|Permission"
```

### Desktop Development (schneller für UI-Arbeit)

```bash
dx serve --platform desktop
```

**Hinweis:** Camera/Gallery funktioniert nur auf Android (JNI-basiert).

## 📱 Android-Spezifika

### Permissions (Android 13+ kompatibel)

**Manifest (`android/AndroidManifest.xml`):**
- `CAMERA` (runtime)
- `READ_MEDIA_IMAGES` (Android 13+, runtime)
- `READ_EXTERNAL_STORAGE` (maxSdkVersion 32, legacy)
- `WRITE_EXTERNAL_STORAGE` (maxSdkVersion 28, legacy)
- `INTERNET` (für zukünftige Features)

**Runtime Permission Flow:**
1. Rust ruft `camera::capture_photo()` oder `camera::pick_image()`
2. JNI-Bridge checkt Permission via `MainActivity.hasCameraPermission()` / `hasStoragePermission()`
3. Falls fehlend: `requestCameraPermission()` / `requestStoragePermission()` → Android-Dialog
4. Nach Grant: Intent startet (`ACTION_IMAGE_CAPTURE` oder `ACTION_GET_CONTENT`)
5. Ergebnis via `ActivityResultLauncher` → `lastPhotoPath` → Rust polling

### Custom MainActivity

**Warum notwendig?**
- Standard Dioxus `WryActivity` unterstützt keine `ActivityResultLauncher` für Camera/Gallery
- Custom Activity erweitert `WryActivity` und fügt Intent-Handling hinzu

**Key Components:**
```kotlin
class MainActivity : WryActivity() {
    private lateinit var cameraLauncher: ActivityResultLauncher<Uri>
    private lateinit var galleryLauncher: ActivityResultLauncher<String>
    
    companion object {
        @JvmStatic var instance: MainActivity? = null
        @JvmStatic var lastPhotoPath: String? = null
        @JvmStatic var lastError: String? = null
    }
}
```

**JNI-Zugriff (Rust → Kotlin):**
```rust
// camera.rs
let cls = load_class(&mut env, "dev/dioxus/main/MainActivity")?;
let (activity, _) = get_activity_instance(&mut env)?;
env.call_method(activity, "launchCamera", "()V", &[])?;
```

### FileProvider (für Camera)

**Config (`android/res/xml/file_paths.xml`):**
```xml
<external-cache-path name="my_images" path="/" />
```

**Authority:** `de.teilgedanken.stalltagebuch.fileprovider`

Temporary Kamera-Fotos werden in `getExternalCacheDir()` gespeichert.

## 🧪 Testing

### Unit Tests (Services)

```bash
cargo test
```

**Coverage:**
- `profile_service`: CRUD Operations
- `egg_service`: CRUD + Date Handling
- `analytics_service`: Statistik-Berechnungen

### On-Device Testing

1. Build & Install (siehe oben)
2. App öffnen
3. **Profile erstellen:** Navigation → "Profile" → "+" Button
4. **Kamera testen:** Profil → Kamera-Icon → Permission-Dialog → Foto aufnehmen
5. **Galerie testen:** Profil → Galerie-Icon → Permission-Dialog → Bild auswählen
6. **Eier erfassen:** Navigation → "Eier Tracking" → Datum & Anzahl eingeben
7. **Statistik prüfen:** Navigation → "Statistik" → Zeitraum-Filter (Alle/Woche/Monat/Jahr)

## 🐛 Bekannte Probleme & Lösungen

### Build-Warnings (können ignoriert werden)

- **Java source/target 8 deprecated:** Legacy-Einstellung von Dioxus-generiertem Gradle-File
- **extractNativeLibs in Manifest:** AGP-Warnung (funktional korrekt)
- **BuildConfig feature deprecated:** Harmlos, wird durch Typealias umgangen

### Runtime-Fehler

**ClassNotFoundException: dev.dioxus.main.MainActivity**
→ **Lösung:** Nutze `build_android.sh` statt `dx build` allein (Script copied MainActivity korrekt)

**Camera/Gallery-Crash bei Permission-Denial**
→ **Lösung:** Implementiert in `MainActivity` Permission-Checks vor Intent-Launch

**JNI FindClass fails on native thread**
→ **Lösung:** Nutze Application ClassLoader (siehe `camera::get_app_class_loader()`)

## 📚 Dokumentation

- **[DEVELOPMENT.md](DEVELOPMENT.md):** Build-Anleitung und Testing
- **[AGENTS.md](AGENTS.md):** Dioxus 0.7 API-Referenz für AI-Assistenten
- **[PROPOSALS.md](PROPOSALS.md):** Geplante Features

## 📄 Lizenz
MIT or Apache2

---

**Version:** 0.1.0  
**Letzte Aktualisierung:** 2025-11-09  
**Rust:** 1.83+ | **Dioxus:** 0.7.1 | **Min Android:** API 28 (Android 9) | **Target:** API 34 (Android 14)

