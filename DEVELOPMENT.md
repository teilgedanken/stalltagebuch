# Development Guide

## Voraussetzungen

- **Rust** 1.83+ mit Android-Targets
- **Dioxus CLI** 0.7+
- **Android NDK** 26.1+
- **Android SDK** API 34
- **adb** (Android Debug Bridge)

## Setup

### Rust und Targets

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-linux-android aarch64-linux-android
```

### Dioxus CLI

```bash
curl -sSL http://dioxus.dev/install.sh | sh
```

### Android SDK/NDK

Setze `ANDROID_HOME` und `NDK_HOME` Umgebungsvariablen. Details siehe [Android Developer Docs](https://developer.android.com/studio/command-line/sdkmanager).

## Build

Since Dioxus 0.7, custom Android manifests and MainActivity are natively supported through `Dioxus.toml` configuration:

```toml
[application]
android_manifest = "./android/AndroidManifest.xml"
android_main_activity = "./android/MainActivity.kt"
android_min_sdk_version = 28

[bundle]
identifier = "de.teilgedanken.stalltagebuch"
```

### Debug Build

```bash
# Simple build (standard Dioxus CLI)
dx build --platform android

# Or use the wrapper script which also copies file_paths.xml for FileProvider/Camera functionality
./build_android.sh
```

### Release Build

```bash
./build_android.sh --release
```

Lint-Tasks für Release-Builds werden automatisch übersprungen, damit der Gradle-Prozess nicht an `lintVital` scheitert.

**APK-Pfad (signiert):**
```
target/dx/stalltagebuch/release/android/app/app/build/outputs/apk/release/app-release.apk
```

### Installation

```bash
adb install -r target/dx/stalltagebuch/debug/android/app/app/build/outputs/apk/debug/app-debug.apk
```

Für Release-Builds ersetze den Pfad durch `release/.../app-release.apk`.

### Signing

Release-Builds werden automatisch mit dem Entwicklungs-Keystore signiert:

| Feld | Wert |
| --- | --- |
| Keystore | `android/release.keystore` |
| Alias | `stalltagebuchReleaseKey` |
| Passwörter | `android` |

Für produktive Uploads sollte ein eigener Keystore erstellt und die Werte in `Dioxus.toml` (`[bundle.android]`) angepasst werden.

### Desktop Development (schneller)

```bash
dx serve --platform desktop
```

**Hinweis:** Camera/Gallery funktioniert nur auf Android.

## Android Emulator

### AVD erstellen

```bash
avdmanager create avd \\
    -n Medium_Phone_API_36 \\
    -k "system-images;android-36.1;google_apis_playstore;x86_64" \\
    -d "medium_phone_api_36"
```

### Emulator starten

```bash
emulator -avd Medium_Phone_API_36 &
adb wait-for-device
```

### Emulator verifizieren

```bash
adb devices
adb shell getprop ro.build.version.release
```

## Testing

### App starten

```bash
adb shell am start -n de.teilgedanken.stalltagebuch/dev.dioxus.main.MainActivity
```

### Live-Logs

```bash
adb logcat | grep -i stalltagebuch
adb logcat *:E | grep -i stalltagebuch  # Nur Errors
```

### Datenbank inspizieren

```bash
adb shell run-as de.teilgedanken.stalltagebuch cp /data/data/de.teilgedanken.stalltagebuch/files/stalltagebuch.db /sdcard/
adb pull /sdcard/stalltagebuch.db .
sqlite3 stalltagebuch.db
```

## Troubleshooting

### Camera/Gallery crashed

→ Prüfe Permissions in Settings: Apps → Stalltagebuch

### Build-Fehler

```bash
# NDK prüfen
ls \$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android*-clang

# ANDROID_HOME prüfen
echo \$ANDROID_HOME

# ADB neu starten
adb kill-server
adb start-server
```

## Development Workflow

```bash
# 1. Code ändern
vim src/components/home.rs

# 2. Rebuild & Install
./build_android.sh && adb install -r target/dx/stalltagebuch/debug/android/app/app/build/outputs/apk/debug/app-debug.apk

# 3. Logs beobachten
adb logcat | grep -i stalltagebuch
```

## Ressourcen

- **Dioxus Docs:** https://dioxuslabs.com/learn/0.7
- **Android Developer:** https://developer.android.com/
- **JNI Guide:** https://docs.rs/jni/latest/jni/
- **SpacetimeDB Docs:** https://spacetimedb.com/docs

---

## SpacetimeDB Backend

Starting with this PR the primary backend is SpacetimeDB instead of local SQLite + WebDAV sync.

### Architecture

| Layer | Role |
|-------|------|
| `stalltagebuch-server/` | SpacetimeDB server module (Rust → WASM). Defines tables (`quails`, `quail_events`, `egg_records`) and reducers. Deployed once per environment. |
| `src/spacetime/` | Client-side integration. `SpacetimeClient` calls the SpacetimeDB HTTP API; `SpacetimeContext` wraps table data in Dioxus `Signal<Vec<...>>`. |
| Nextcloud WebDAV | Photo storage only (unchanged). |

### Deploying the server module

```bash
# Install the SpacetimeDB CLI (see https://spacetimedb.com/install – verify the script)
spacetime login

# From the repo root:
spacetime publish --project-path stalltagebuch-server stalltagebuch-server
```

### Generating typed client bindings

Once the Dioxus binding generator is merged (https://github.com/enaut/SpacetimeDB/pull/4):

```bash
spacetime generate \
  --lang dioxus \
  --out-dir src/spacetime/module_bindings \
  --project-path stalltagebuch-server
```

Until then, the hand-written types in `src/spacetime/types.rs` and the context in
`src/spacetime/context.rs` serve as the reactive layer.

### Connecting the app

1. Open Settings in the app.
2. Fill in **SpacetimeDB server URL**, **database name**, and **auth token**.
3. Tap **Save & Connect** – the app will load all quails, events and egg records from the server.

### Transition period

The local SQLite database is still initialised at startup for the photo-gallery crate.
The old sync services (`sync_service`, `crdt_service`, `background_sync`, etc.) remain in the
codebase but are no longer started automatically; they will be removed in a follow-up once the
SpacetimeDB integration is fully validated.

---

## Experimental Sync Integration Plan (Nextcloud/WebDAV – superseded)

Ziel: Multi‑Master, Offline‑First Sync ohne Konflikte. Umsetzung stufenweise hinter Feature‑Flag `experimental_sync`.

### Code‑Integrationspunkte
- Services
  - `src/services/sync_service.rs`: Pull (PROPFIND/GET), Manifestpflege (ETag), Replay‑Pipeline, Snapshot‑Nutzung
  - `src/services/upload_service.rs`: Batch‑Upload neuer NDJSON‑Dateien, atomar via `If-None-Match: *`
  - Neu: `src/services/crdt_service.rs` (geplant): HLC, Feld‑CRDTs (LWW/OR‑Set/PN‑Counter), Merge API
- Datenbank/Schema
  - `src/database/schema.rs`: additive Spalten `rev INTEGER`, `logical_clock INTEGER`, `deleted INTEGER` je Entität; Tabellen `op_log`, `device_state`, `sync_checkpoint`
  - Migrations sind additive und rückwärtskompatibel; keine Legacy‑Änderungen entfernen
- Modelle
  - `src/models/*.rs`: stabile `id` (ULID/UUIDv7), `deleted: bool`, optionale `rev`/`logical_clock`
- UI/Komponenten
  - `src/components/settings.rs`: Schalter „Experimental Sync“, Anzeige Device‑ID, letzter Merge/Snapshot
  - Neu: `src/components/sync_diagnostics.rs` (geplant): ausstehende Ops, letzte Fehler, Rebuild/Resync Aktionen
- i18n
  - `locales/de-DE.ftl`, `locales/en-US.ftl`: Schlüssel wie `sync-experimental`, `sync-device-id`, `sync-resync`, `sync-diagnostics`, `sync-migration-running`

### Stufenweiser Rollout
1. Vorbereitung
    - Feature‑Flag `experimental_sync` (default OFF)
    - Device‑ID generieren und persistent speichern
2. Shadow Logging
    - Lokale Operationen zusätzlich als NDJSON batchen; Legacy‑Upload unverändert
3. Dry‑Run Pull/Merge
    - Remote ops/ lesen, Merge simulieren, nur Diagnose anzeigen
4. Aktives Pull→Apply
    - Merge anwenden, lokale DB aktualisieren; Push weiterhin Legacy
5. Aktiver Push
    - NDJSON‑Batches hochladen; `snapshots/` erzeugen; `latest.json` mit `If-Match` aktualisieren
6. GA & Sunset
    - Standard ON; Legacy‑Pfad später entfernen, wenn Metriken stabil

### Risiken & Mitigation
- Große Verzeichnisse: Segmentierung nach Monat (`YYYYMM`), Manifeste cachen
- ETag‑Unstetigkeit: Sekundäre Prüfung via Größe/Zeitstempel; vollständiger Re‑Scan als Fallback
- Zeitdrift: HLC nutzt logischen Counter; Tiebreak per `device_id`
- Manuelle Servereingriffe: Validierung, Quarantäne‑Pfad, Diagnosemeldungen
- Rollback: Flag OFF → Legacy‑Sync aktiv, Logs/Snapshots werden ignoriert

Siehe Details in `SYNC_FORMAT.md` (Abschnitt „Experimental Multi‑Master Sync“).
