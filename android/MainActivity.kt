package dev.dioxus.main

import android.Manifest
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import java.io.File
import java.text.SimpleDateFormat
import java.util.*

/**
 * MainActivity für Stalltagebuch
 *
 * Diese Klasse erweitert die vom Dioxus/Wry-Wrapper bereitgestellte `WryActivity`
 * und stellt plattform-spezifische Android-Funktionen bereit, die von der Rust-Seite
 * per JNI aufgerufen werden können:
 *
 * Hauptverantwortlichkeiten:
 * - Bereitstellung von Camera-Integration (Foto aufnehmen) via Android `ActivityResult`.
 * - Bereitstellung von Gallery-Integration (Dateiauswahl, single und multi) via `ActivityResult`.
 * - Verwaltung temporärer Dateien/URIs über `FileProvider` und sichere Ablage im App-spezifischen Verzeichnis.
 * - Permission-Handling (Kamera, Storage) mit Unterstützung für moderne Android-Versionen
 *   (z. B. READ_MEDIA_IMAGES ab API 33 / Android 13).
 * - Austausch von Ergebnissen über statische `@JvmStatic`-Methoden und `@Volatile` Felder
 *   damit die Rust-Seite (oder andere Java/Kotlin-Klassen) synchron auf das Ergebnis zugreifen können.
 *
 * Thread-safety & JNI:
 * - Die Felder `currentPhotoPath`, `currentPhotoPaths` und `lastError` sind `@Volatile`, um
 *   einfache, thread-sichere Kommunikation zwischen UI-Thread und anderen Threads/JNI-Calls zu ermöglichen.
 * - Die `@JvmStatic`-Getter erleichtern native Aufrufern (Rust/JNI) den Zugriff auf die zuletzt
 *   erstellten Pfade / Fehlertexte. Der Aufrufer sollte die Werte prüfen und ggf. auf `null` reagieren.
 *
 * Bemerkung zur Architektur:
 * - Diese Activity kapselt alle Android-spezifischen Details (Storage, FileProvider, Permissions).
 * - App-logik, Datenhaltung und UI bleiben im Rust/Dioxus-Code; die Activity bleibt so dünn wie möglich
 *   und bietet nur OS-spezifische Implementationen an.
 */
class MainActivity : WryActivity() {
    
    companion object {
        private const val CAMERA_PERMISSION_CODE = 1001
        private const val STORAGE_PERMISSION_CODE = 1002
        
        // Singleton-Referenz auf die Activity
        // Exposed as a JVM static field to make reflective/JNI access robust against
        // method removal/obfuscation by R8. Using @JvmField ensures a direct static
        // field `instance` exists on the outer class `MainActivity` which native code
        // can fetch using `getStaticField` reliably.
        @JvmField
        @Volatile
        var instance: MainActivity? = null
        
        @JvmStatic
        fun getInstance(): MainActivity? = instance
        
        // Static variables für JNI-Zugriff
        // Diese Felder sind bewusst `@Volatile` markiert um einfachen sicheren Zugriff
        // aus anderen Threads (inkl. JNI/Rust-Seite) zu ermöglichen. Sie repräsentieren
        // das Ergebnis der zuletzt ausgeführten Camera/Gallery-Aktionen.
        @Volatile
        private var currentPhotoPath: String? = null
        @Volatile
        private var currentPhotoPaths: String? = null // Newline-separated paths for multi-select
        
        @Volatile
        private var lastError: String? = null
        
        /**
         * Gibt den Pfad der zuletzt aufgenommenen/ausgewählten Einzeldatei zurück.
         * - `null` bedeutet: kein Ergebnis / Fehler oder Abbruch.
         * Aufrufbar aus Rust via JNI (statischer Methodenzugriff).
         */
        @JvmStatic
        fun getLastPhotoPath(): String? = currentPhotoPath
        
        /**
         * Liefert den letzten Fehlertext (falls ein Fehler aufgetreten ist).
         * - Fehlertexte sind kurze, benutzerfreundliche Meldungen in Deutsch.
         */
        @JvmStatic
        fun getLastError(): String? = lastError
        
        /**
         * Liefert Mehrfach-Pfade als newline-separierten String.
         * - Verwenden: `currentPhotoPaths?.split("\n")` um die einzelnen Pfade zu erhalten.
         */
        @JvmStatic
        fun getLastPhotoPaths(): String? = currentPhotoPaths
        
        /**
         * Setzt den letzten Fehler zurück. Kann von JNI-Aufrufern genutzt werden,
         * um Fehlerzustand nach erfolgreichem Lesen zu löschen.
         */
        @JvmStatic
        fun clearLastError() {
            lastError = null
        }
    }
    
    // ActivityResultLauncher für Gallery-Auswahl (single)
    // - `GetContent()` wird verwendet um eine einzelne Datei auszuwählen und
    //   über ein Content-URI zu liefern. Die Activity kopiert die Datei anschließend
    //   in einen app-internen Speicherort und speichert den absoluten Pfad in `currentPhotoPath`.
    private lateinit var pickImageLauncher: ActivityResultLauncher<String>
    // ActivityResultLauncher für Gallery-Auswahl (multiple)
    // - `GetMultipleContents()` erlaubt die Mehrfachauswahl. Wir kopieren alle ausgewählten
    //   Dateien in den internen App-Ordner und geben die Pfade als newline-separierten String zurück.
    private lateinit var pickImagesLauncher: ActivityResultLauncher<String>
    
    // ActivityResultLauncher für Kamera
    // - `TakePicture()` benötigt eine URI (z. B. über FileProvider) in die die Kamera-App schreibt.
    private lateinit var takePictureLauncher: ActivityResultLauncher<Uri>
    
    // Temporäre URI für Kamera-Foto
    private var photoUri: Uri? = null
    
    // Pending action nach Permission-Grant
    // - Wenn die benötigten Berechtigungen noch nicht erteilt sind, setzen wir `pendingAction`
    //   auf eine lambda, die die eigentliche Aktion ausführt (z. B. `launchCameraInternal`).
    //   Nach Benutzerentscheidung (Permission granted) wird diese Aktion automatisch ausgeführt.
    private var pendingAction: (() -> Unit)? = null
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        instance = this
        
        // Register Gallery-Picker
        // - Die registered callback-Lambdas laufen auf dem UI-Thread. Sie müssen deshalb
        //   schnell und nicht-blockierend sein; schwere Arbeit sollte in background threads ausgelagert werden.
        pickImageLauncher = registerForActivityResult(
            ActivityResultContracts.GetContent()
        ) { uri: Uri? ->
            if (uri != null) {
                // Kopiere ausgewähltes Bild in internen Speicher
                // - Wir nutzen `createImageFile()` um eine sichere, eindeutige Datei in`getExternalFilesDir("photos")` / `filesDir` zu erstellen.
                // - Die App kontrolliert die resultierende Datei und setzt `currentPhotoPath` auf den absoluten Dateipfad.
                try {
                    val photoFile = createImageFile()
                    contentResolver.openInputStream(uri)?.use { input ->
                        photoFile.outputStream().use { output ->
                            input.copyTo(output)
                        }
                    }
                    // Speichere den absoluten Pfad zur neu erzeugten Datei. JNI-Aufrufer können diesen Pfad lesen.
                    currentPhotoPath = photoFile.absolutePath
                    currentPhotoPaths = null
                    lastError = null
                } catch (e: Exception) {
                    lastError = "Fehler beim Kopieren des Bildes: ${e.message}"
                    currentPhotoPath = null
                    currentPhotoPaths = null
                }
            } else {
                // Benutzer hat den Dateiauswahl-Dialog abgebrochen ohne Auswahl
                lastError = "Keine Datei ausgewählt"
                currentPhotoPath = null
                currentPhotoPaths = null
            }
        }
        
        // Register Gallery-Picker (multiple)
        pickImagesLauncher = registerForActivityResult(
            ActivityResultContracts.GetMultipleContents()
        ) { uris: List<Uri> ->
            if (!uris.isNullOrEmpty()) {
                // Wurde mindestens ein URI ausgewählt?
                // - Jede URI wird in eine neue temporäre Datei kopiert. Die Liste der Pfade
                //   wird anschließend newline-separiert in `currentPhotoPaths` gespeichert.
                try {
                    val paths = mutableListOf<String>()
                    for (uri in uris) {
                        val photoFile = createImageFile()
                        contentResolver.openInputStream(uri)?.use { input ->
                            photoFile.outputStream().use { output ->
                                input.copyTo(output)
                            }
                        }
                        paths.add(photoFile.absolutePath)
                    }
                    // Mehrfach-Auswahl: Pfade als newline-separierten String bereitstellen.
                    currentPhotoPaths = paths.joinToString("\n")
                    currentPhotoPath = null
                    lastError = null
                } catch (e: Exception) {
                    lastError = "Fehler beim Kopieren der Bilder: ${e.message}"
                    currentPhotoPaths = null
                    currentPhotoPath = null
                }
            } else {
                lastError = "Keine Dateien ausgewählt"
                currentPhotoPaths = null
                currentPhotoPath = null
            }
        }
        
        // Register Kamera
        takePictureLauncher = registerForActivityResult(
            ActivityResultContracts.TakePicture()
        ) { success: Boolean ->
            if (success && currentPhotoFile != null) {
                // Foto wurde erfolgreich aufgenommen
                // - Die Kamera-App hat in die mittels FileProvider übergebene Datei geschrieben.
                // - `currentPhotoFile` wurde zuvor in `launchCameraInternal()` erstellt und kann jetzt
                //   direkt als erfolgreiches Ergebnis verwendet werden.
                // - Nur der absolute Pfad wird zurückgegeben; `currentPhotoPaths` bleibt `null`.
                currentPhotoPath = currentPhotoFile?.absolutePath
                currentPhotoPaths = null
                lastError = null
            } else {
                // Kamera-Aktion wurde abgebrochen oder war nicht erfolgreich.
                lastError = "Foto-Aufnahme abgebrochen oder fehlgeschlagen"
                currentPhotoPath = null
                currentPhotoPaths = null
            }
            currentPhotoFile = null
        }
    }
    
    override fun onDestroy() {
        super.onDestroy()
        if (instance == this) {
            instance = null
        }
    }
    
    /**
     * Prüft ob die App die Camera-Permission besitzt.
     * - Gibt `true` zurück wenn `Manifest.permission.CAMERA` gesetzt ist.
     */
    fun hasCameraPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.CAMERA
        ) == PackageManager.PERMISSION_GRANTED
    }
    
    /**
     * Prüft ob die App Lese-Rechte für Images besitzt.
     * - Auf Android 13+ (API 33) nutzen wir `READ_MEDIA_IMAGES`.
     * - Auf älteren Android-Versionen prüfen wir `READ_EXTERNAL_STORAGE`.
     *
     * Hinweis: Da wir die ausgewählten Inhalte in unseren App-spezifischen Speicher kopieren,
     * benötigen wir bloß Leserechte für die Quelle. Auf neueren Android-Versionen sind granulare
     * Berechtigungen vorhanden (READ_MEDIA_IMAGES).
     */
    fun hasStoragePermission(): Boolean {
        // Ab Android 13 (API 33) gibt es READ_MEDIA_IMAGES statt READ_EXTERNAL_STORAGE
        return if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.READ_MEDIA_IMAGES
            ) == PackageManager.PERMISSION_GRANTED
        } else {
            ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.READ_EXTERNAL_STORAGE
            ) == PackageManager.PERMISSION_GRANTED
        }
    }
    
    /**
     * Fordert die Kamera-Berechtigung beim Nutzer an.
     * - Wenn Permission bereits gesetzt ist, löst Android kein Callback mehr aus.
     */
    fun requestCameraPermission() {
        ActivityCompat.requestPermissions(
            this,
            arrayOf(Manifest.permission.CAMERA),
            CAMERA_PERMISSION_CODE
        )
    }
    
    /**
     * Fordert die nötigen Storage-Berechtigungen beim Nutzer an.
     * - Auf Android 13+ fordern wir `READ_MEDIA_IMAGES`; älter: `READ_EXTERNAL_STORAGE` + `WRITE_EXTERNAL_STORAGE`.
     * - `WRITE_EXTERNAL_STORAGE` ist für scoped storage nicht immer notwendig, aber zur Kompatibilität
     *   für ältere Geräte hier inkludiert.
     */
    fun requestStoragePermission() {
        val permissions = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.TIRAMISU) {
            arrayOf(Manifest.permission.READ_MEDIA_IMAGES)
        } else {
            arrayOf(
                Manifest.permission.READ_EXTERNAL_STORAGE,
                Manifest.permission.WRITE_EXTERNAL_STORAGE
            )
        }
        
        ActivityCompat.requestPermissions(
            this,
            permissions,
            STORAGE_PERMISSION_CODE
        )
    }
    
    /**
     * Callback nachdem Benutzer auf einen Permission-Dialog reagiert hat.
     * - Wird sowohl für Kamera- als auch Storage-Permissions verwendet.
     * - Bei Erfolg: Falls eine `pendingAction` gesetzt war, wird diese ausgeführt und gecleart.
     * - Bei Ablehnung: `lastError` wird gesetzt um den Aufrufer zu informieren.
     */
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        
        when (requestCode) {
            CAMERA_PERMISSION_CODE -> {
                if (grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
                    // Permission granted — führe gegebenenfalls die zuvor wartende Aktion aus
                    pendingAction?.invoke()
                    pendingAction = null
                } else {
                    // Permission verweigert — vermerke den Fehler für Aufrufer
                    lastError = "Kamera-Berechtigung verweigert"
                }
            }
            STORAGE_PERMISSION_CODE -> {
                if (grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
                    // Permission granted — führe gegebenenfalls die zuvor wartende Aktion aus
                    pendingAction?.invoke()
                    pendingAction = null
                } else {
                    // Permission verweigert — vermerke den Fehler für den Aufrufer
                    lastError = "Speicher-Berechtigung verweigert"
                }
            }
        }
    }
    
    /**
     * Öffne Gallery für Einzeldatei-Auswahl.
     * - Entfernt vorherige Ergebnis-/Fehlerzustände und prüft Permissions.
     * - Falls Permission fehlt, wird `pendingAction` gesetzt und `requestStoragePermission()` aufgerufen.
     * - Wird die Permission nachträglich erteilt, führt die Activity die Aktion automatisch aus.
     *
     * Hinweis: Diese Methode ist so konzipiert, dass sie aus Rust via JNI aufgerufen werden kann.
     */
    fun launchImagePicker() {
        try {
            currentPhotoPath = null
            currentPhotoPaths = null
            lastError = null
            
            if (!hasStoragePermission()) {
                pendingAction = { launchImagePickerInternal() }
                requestStoragePermission()
            } else {
                launchImagePickerInternal()
            }
        } catch (e: Exception) {
            lastError = "Fehler beim Öffnen der Gallery: ${e.message}"
        }
    }
    
    // Private Hilfsfunktion: Startet tatsächlich den single-file Picker.
    private fun launchImagePickerInternal() {
        pickImageLauncher.launch("image/*")
    }
    
    /**
     * Öffne Gallery für Mehrfachauswahl.
     * - Gleiches Verhalten wie `launchImagePicker()` aber mit Multi-Select-Unterstützung.
     */
    fun launchImagePickerMulti() {
        try {
            currentPhotoPath = null
            currentPhotoPaths = null
            lastError = null
            
            if (!hasStoragePermission()) {
                pendingAction = { launchImagePickerMultiInternal() }
                requestStoragePermission()
            } else {
                launchImagePickerMultiInternal()
            }
        } catch (e: Exception) {
            lastError = "Fehler beim Öffnen der Gallery (multi): ${e.message}"
        }
    }
    
    // Private Hilfsfunktion: Startet den multi-file Picker.
    private fun launchImagePickerMultiInternal() {
        pickImagesLauncher.launch("image/*")
    }
    
    /**
     * Öffne die Kamera um ein neues Foto aufzunehmen.
     * - Erstellt intern eine temporäre Datei, übergibt deren URI via `FileProvider` an die Kamera-App
     *   und verwendet `TakePicture()` ActivityResult, um festzustellen ob das Foto erfolgreich war.
     */
    fun launchCamera() {
        try {
            currentPhotoPath = null
            lastError = null
            
            if (!hasCameraPermission()) {
                pendingAction = { launchCameraInternal() }
                requestCameraPermission()
            } else {
                launchCameraInternal()
            }
        } catch (e: Exception) {
            lastError = "Fehler beim Öffnen der Kamera: ${e.message}"
        }
    }
    
    // Referenz auf die temporär erzeugte Zieldatei, in die die Kamera-App schreibt.
    // Wird nur während der laufenden Aufnahme verwendet. Nach Abschluss wird das Objekt gecleart.
    private var currentPhotoFile: File? = null
    
    // Interner Helfer: Erzeugt die Zieldatei, den FileProvider-URI und startet die Kamera.
    // - FileProvider stellt sicher, dass wir keine direkten File-URIs zwischen Apps teilen.
    private fun launchCameraInternal() {
        try {
            // Erstelle temporäre Datei für Foto
            val photoFile = createImageFile()
            currentPhotoFile = photoFile
            
            // Erstelle URI mit FileProvider
            // - Verwende packageName.fileprovider (siehe manifest und res/xml/file_paths.xml).
            // - FileProvider sorgt für die nötigen temporären Berechtigungen für die Kamera-App.
            photoUri = FileProvider.getUriForFile(
                this,
                "${packageName}.fileprovider",
                photoFile
            )
            
            // Starte Kamera mit URI. Das Resultat wird in takePictureLauncher verarbeitet.
            takePictureLauncher.launch(photoUri)
            
        } catch (e: Exception) {
            lastError = "Fehler beim Starten der Kamera: ${e.message}"
        }
    }
    
    /**
     * Erstelle eindeutige Datei für Foto
     *
     * Verhalten & Gründe:
     * - Dateien werden in `getExternalFilesDir("photos")` abgelegt wenn verfügbar. Diese Location
     *   ist App-privat und wird bei App-Deinstallation entfernt. Falls `null` (unwahrscheinlich),
     *   nutzen wir `filesDir` als Fallback.
     * - Wir erzeugen eine eindeutige Datei mit Präfix `WACHTEL_yyyyMMdd_HHmmss_` um Kollisionen
     *   zwischen mehreren Aufnahmen zu vermeiden.
     * - `createTempFile` sorgt für eine sichere Dateierzeugung mit eindeutigem Namen.
     */
    private fun createImageFile(): File {
        val timestamp = SimpleDateFormat("yyyyMMdd_HHmmss", Locale.getDefault()).format(Date())
        val storageDir = getExternalFilesDir("photos") ?: filesDir
        
        if (!storageDir.exists()) {
            storageDir.mkdirs()
        }
        
        return File.createTempFile(
            "WACHTEL_${timestamp}_",
            ".jpg",
            storageDir
        )
    }
}
