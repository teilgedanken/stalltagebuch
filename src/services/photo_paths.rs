use std::path::{Path, PathBuf};

const SMALL_THUMB_SUFFIX: &str = "_128.webp";
const MEDIUM_THUMB_SUFFIX: &str = "_512.webp";

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

pub fn small_thumbnail_relative_path(uuid: &str) -> String {
    format!("{uuid}{SMALL_THUMB_SUFFIX}")
}

pub fn medium_thumbnail_relative_path(uuid: &str) -> String {
    format!("{uuid}{MEDIUM_THUMB_SUFFIX}")
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

pub fn derive_uuid_from_path(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy();
    let name = file_name.as_ref();

    if let Some(stem) = name.strip_suffix(SMALL_THUMB_SUFFIX) {
        return Some(stem.to_string());
    }
    if let Some(stem) = name.strip_suffix(MEDIUM_THUMB_SUFFIX) {
        return Some(stem.to_string());
    }
    if let Some(stem) = name.strip_suffix(".jpg") {
        return Some(stem.to_string());
    }
    if let Some(stem) = name.strip_suffix(".jpeg") {
        return Some(stem.to_string());
    }

    None
}
