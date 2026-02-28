# Copilot Projekt-Instruktionen (stalltagebuch)

Ziel: Native Android-Dioxus-0.7 App (Nur Android) zur Verwaltung von Wachteln, Ereignissen & Eierproduktion mit lokaler SQLite + optionaler Nextcloud Sync.

## Architektur & Muster
- Navigation: Enum `Screen` in `src/main.rs`, Wechsel über `Signal<Screen>` und Props `on_navigate: Fn(Screen)` in Komponenten.
- UI Komponenten: In `src/components/*`; jede Screen-Komponente akzeptiert eigene Props + einen `on_navigate` Callback. Keine veralteten APIs (`cx`, `Scope`, `use_state`); benutze `use_signal`, `use_memo`, `use_effect`.
- Services: Fachlogik unter `src/services/*` (z.B. `profile_service`, `egg_service`, `analytics_service`, Sync/CRDT in `crdt_service`, `background_sync`). Schreib neue Logik hier – Screens bleiben dünn.
- Modelle: `src/models/*` als reine Datenstrukturen (Owned Types, `PartialEq`, `Clone` wenn als Props genutzt).
- Datenbank: Schema + Migration in `database/schema.rs` – CRDT Felder (`rev`, `logical_clock`, `deleted`) und `op_log`/`sync_queue` für Sync. Änderungen: neue Migration statt direktes Anpassen bestehender CREATE.
- Sync: Settings via `sync_settings` Tabelle; Autostart in `App` über `use_effect` nach `init_database`.
- JNI/Android: Kamera & Galerie über `camera.rs` + `android/MainActivity.kt`; nutze ClassLoader-Helper (siehe `camera.rs`). Führe keinen direkten Zugriff auf Android APIs außerhalb dieser Brücke ein.

## Build & Workflow
- nutze IMMER `./build_android.sh`. Nutze NIE `dx build`;  Nutze auch NIE `cargo check`; das Script build_android.sh patcht Gradle, kopiert Manifest/Activity/Res und setzt Java 17.
- do NOT wrap to commands into additional bash or similar shells; just execute them directly!
- Installiere APK danach mit: `adb install -r target/dx/stalltagebuch/debug/android/app/app/build/outputs/apk/debug/app-debug.apk`
- Logcat Filter: `adb logcat | grep -iE "stalltagebuch|MainActivity"`

## Internationalisierung
- Keys generieren: `dx-i18n -o locales/de-DE.ftl` (nicht manuell neue Keys direkt in Datei hinzufügen – nur Übersetzungen ergänzen). Nutze vorhandene i18n Initialisierung: `use_init_i18n(i18n::init_i18n);`.

## Stil & Konventionen
- Fehler: Zentral in `error.rs` erweitern statt ad-hoc `eprintln!` Streuung – vorhandene Pattern respektieren.
- Neue Datenbankfelder: Achte auf passende Indexe + `updated_at` Trigger wenn notwendig.

## Beispiele
```rust
let mut current_screen = use_signal(|| Screen::Home);
NavigationBar { current_screen: current_screen(), on_navigate: move |s| current_screen.set(s) }
```
```rust
// Service-Aufruf in Screen
let quails = services::profile_service::list_quails(&conn)?;
```

## Prüfliste für neue Features
1. Datenmodell erweitern (models + Migration)
2. Service-Funktion ergänzen (CRUD/Analyse)
3. UI Screen + Navigation Callback
4. i18n Key erzeugen & Übersetzung nachziehen
5. Android-spezifisch? → Anpassung nur über bestehende JNI-Brücke
6. format code: `cargo fmt`
7. Build mit Script & testen auf Emulator
8. Check if changes affect this file or other instruction files and update `./.github/copilot-instructions.md` accordingly