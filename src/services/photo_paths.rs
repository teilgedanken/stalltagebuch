use std::path::PathBuf;

pub fn photo_storage_root() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from("/storage/emulated/0/Android/data/de.teilgedanken.stalltagebuch/files/photos")
    }

    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./photos")
    }
}

pub fn original_relative_path(uuid: &str) -> String {
    format!("{uuid}.jpg")
}

pub fn original_absolute_path(uuid: &str) -> PathBuf {
    photo_storage_root().join(original_relative_path(uuid))
}

pub fn relative_to_absolute(relative_path: &str) -> PathBuf {
    photo_storage_root().join(relative_path)
}

pub fn ensure_photo_storage_dir() -> Result<(), std::io::Error> {
    std::fs::create_dir_all(photo_storage_root())
}
