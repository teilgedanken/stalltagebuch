use crate::error::AppError;
use base64::Engine;
use std::path::{Path, PathBuf};

// Note: Image processing dependencies will be added in Phase 2.3
// For now, this is a placeholder implementation

/// Process a photo: resize and create thumbnail
/// Returns (main_photo_path, thumbnail_path)
#[allow(dead_code)]
pub fn process_photo(
    input_path: &Path,
    output_dir: &Path,
    filename: &str,
) -> Result<(PathBuf, PathBuf), AppError> {
    // TODO: Implement with `image` crate
    // 1. Load image from input_path
    // 2. Resize to max 1024x1024 (maintain aspect ratio)
    // 3. Save to output_dir/photos/filename
    // 4. Create thumbnail (256x256)
    // 5. Save to output_dir/thumbnails/filename

    let main_path = output_dir.join("photos").join(filename);
    let thumb_path = output_dir.join("thumbnails").join(filename);

    // Placeholder: just copy the file
    std::fs::create_dir_all(main_path.parent().unwrap())?;
    std::fs::create_dir_all(thumb_path.parent().unwrap())?;

    std::fs::copy(input_path, &main_path)?;
    std::fs::copy(input_path, &thumb_path)?;

    Ok((main_path, thumb_path))
}

/// Determines a simple MIME type based on file extension
fn guess_mime_from_ext(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("heic") | Some("heif") => "image/heic",
        _ => "image/jpeg",
    }
}

/// Reads an image from `path` and returns a data URL (Base64)
pub fn image_path_to_data_url(path: &str) -> Result<String, AppError> {
    let p = Path::new(path);
    let mime = guess_mime_from_ext(p);
    let data = std::fs::read(p)
        .map_err(|e| AppError::ImageProcessing(format!("Reading image failed: {}", e)))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(data);
    Ok(format!("data:{};base64,{}", mime, b64))
}

/// Resize an image maintaining aspect ratio
#[allow(dead_code)]
fn calculate_resize_dimensions(
    original_width: u32,
    original_height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    let ratio =
        (original_width as f32 / max_width as f32).max(original_height as f32 / max_height as f32);

    if ratio > 1.0 {
        let new_width = (original_width as f32 / ratio) as u32;
        let new_height = (original_height as f32 / ratio) as u32;
        (new_width, new_height)
    } else {
        (original_width, original_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resize_dimensions() {
        // Image larger than max
        let (w, h) = calculate_resize_dimensions(2000, 1500, 1024, 1024);
        assert!(w <= 1024);
        assert!(h <= 1024);
        assert_eq!(w as f32 / h as f32, 2000.0 / 1500.0); // Maintain aspect ratio

        // Image smaller than max
        let (w, h) = calculate_resize_dimensions(800, 600, 1024, 1024);
        assert_eq!(w, 800);
        assert_eq!(h, 600);
    }
}
