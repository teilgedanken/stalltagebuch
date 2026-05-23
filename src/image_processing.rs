use crate::error::AppError;
use base64::Engine;
use image::{GenericImageView, ImageReader};
use std::path::{Path, PathBuf};

/// Crop rectangle with normalized coordinates (0.0 to 1.0)
/// or pixel coordinates depending on context
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropRect {
    /// X coordinate (normalized 0.0-1.0 or pixel value)
    pub x: f32,
    /// Y coordinate (normalized 0.0-1.0 or pixel value)
    pub y: f32,
    /// Width (normalized 0.0-1.0 or pixel value)
    pub width: f32,
    /// Height (normalized 0.0-1.0 or pixel value)
    pub height: f32,
}

impl CropRect {
    /// Convert normalized coordinates to pixel coordinates
    pub fn to_pixels(&self, img_width: u32, img_height: u32) -> CropRect {
        CropRect {
            x: (self.x * img_width as f32).max(0.0),
            y: (self.y * img_height as f32).max(0.0),
            width: (self.width * img_width as f32)
                .min(img_width as f32 - self.x * img_width as f32),
            height: (self.height * img_height as f32)
                .min(img_height as f32 - self.y * img_height as f32),
        }
    }
}

/// Crop an image file and save it to the output path
///
/// Supports JPEG, PNG, and WebP formats (detects from file extension)
/// Crop rect with normalized coordinates (0.0-1.0) will be converted to pixels
///
/// # Arguments
/// * `input_path` - Path to the image to crop
/// * `output_path` - Path where the cropped image will be saved
/// * `crop_rect` - Crop bounds with normalized coordinates (0.0-1.0)
///
/// # Returns
/// Result with (output_path, width, height) of cropped image
pub fn crop_image(
    input_path: &Path,
    output_path: &Path,
    crop_rect: CropRect,
) -> Result<(PathBuf, u32, u32), AppError> {
    // Load the image
    let img = ImageReader::open(input_path)
        .map_err(|e| AppError::ImageProcessing(format!("Failed to open image: {}", e)))?
        .decode()
        .map_err(|e| AppError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

    let (img_width, img_height) = img.dimensions();

    // Convert normalized crop rect to pixel coordinates
    let pixel_crop = crop_rect.to_pixels(img_width, img_height);

    // Clamp values to valid range
    let x = (pixel_crop.x as u32).min(img_width.saturating_sub(1));
    let y = (pixel_crop.y as u32).min(img_height.saturating_sub(1));
    let w = (pixel_crop.width as u32).min(img_width - x);
    let h = (pixel_crop.height as u32).min(img_height - y);

    if w == 0 || h == 0 {
        return Err(AppError::ImageProcessing(
            "Crop dimensions are invalid or result in empty image".to_string(),
        ));
    }

    // Perform the crop
    let cropped = image::ImageBuffer::from_fn(w, h, |px, py| {
        let img_x = x + px;
        let img_y = y + py;
        img.get_pixel(img_x, img_y).clone()
    });

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Filesystem(e))?;
    }

    // Save the cropped image in the same format as the input
    let format = infer_image_format(input_path)?;

    // Convert to RGB8 if saving as JPEG (JPEG doesn't support alpha channel)
    if format == image::ImageFormat::Jpeg {
        let rgb_image = image::DynamicImage::ImageRgba8(cropped).to_rgb8();
        rgb_image
            .save_with_format(output_path, format)
            .map_err(|e| {
                AppError::ImageProcessing(format!("Failed to save cropped image: {}", e))
            })?;
    } else {
        cropped.save_with_format(output_path, format).map_err(|e| {
            AppError::ImageProcessing(format!("Failed to save cropped image: {}", e))
        })?;
    }

    Ok((output_path.to_path_buf(), w, h))
}

/// Infer image format from file extension
fn infer_image_format(path: &Path) -> Result<image::ImageFormat, AppError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| AppError::ImageProcessing("No file extension found".to_string()))?;

    match ext.as_str() {
        "jpg" | "jpeg" => Ok(image::ImageFormat::Jpeg),
        "png" => Ok(image::ImageFormat::Png),
        "webp" => Ok(image::ImageFormat::WebP),
        _ => Err(AppError::ImageProcessing(format!(
            "Unsupported image format: {}",
            ext
        ))),
    }
}

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
