# Keep MainActivity and all its methods for JNI access
-keep class dev.dioxus.main.MainActivity {
    public *;
    static *;
}

# Keep companion object methods
-keepclassmembers class dev.dioxus.main.MainActivity$Companion {
    public *;
    static *;
}

# Explicitly keep static JNI helpers that are accessed via reflection/JNI
# This prevents R8 from stripping or renaming them if they are only referenced
# reflectively from native code.
-keepclassmembers class dev.dioxus.main.MainActivity {
    public static dev.dioxus.main.MainActivity getInstance();
    public static java.lang.String getLastPhotoPath();
    public static java.lang.String getLastPhotoPaths();
    public static java.lang.String getLastError();
    public static void clearLastError();
}

# Keep the Companion object in case the static accessor isn't present
-keepclassmembers class dev.dioxus.main.MainActivity$Companion {
    public dev.dioxus.main.MainActivity getInstance();
}

# Keep all methods used from JNI
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep FileProvider
-keep class androidx.core.content.FileProvider { *; }
