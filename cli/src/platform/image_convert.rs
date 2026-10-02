//! Makes sure a wallpaper is in a format the desktop displays. WebP (which many Omarchy themes
//! ship) and BMP are converted to PNG once, next to the original, and reused afterwards. The
//! converted file's name and place follow the GUI app on the same OS, so the two share it.

use std::path::{Path, PathBuf};

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertedLayout {
    /// macOS app: `.converted/<file name>.png` beside the original ("1.webp.png").
    Mac,
    /// Windows app: `.<stem>.wallpaper.png` beside the original.
    Windows,
}

#[derive(Debug, thiserror::Error)]
#[error("{0} couldn't be read as an image. Try downloading the theme again.")]
pub struct ImageConversionError(pub String);

#[derive(Debug, Clone, Copy)]
pub struct ImageConverter {
    layout: ConvertedLayout,
}

impl ImageConverter {
    pub fn new(layout: ConvertedLayout) -> Self {
        ImageConverter { layout }
    }

    /// Formats the desktop displays directly (documented for each OS).
    fn is_direct(&self, extension: &str) -> bool {
        let direct: &[&str] = match self.layout {
            ConvertedLayout::Mac => &["png", "jpg", "jpeg", "heic", "heif", "tif", "tiff", "gif"],
            ConvertedLayout::Windows => &["png", "jpg", "jpeg", "bmp"],
        };
        direct.contains(&extension)
    }

    pub fn converted_path(&self, image: &Path) -> PathBuf {
        let dir = image.parent().unwrap_or(Path::new("."));
        let name = image.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match self.layout {
            ConvertedLayout::Mac => dir.join(".converted").join(format!("{name}.png")),
            ConvertedLayout::Windows => {
                let stem = image.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                dir.join(format!(".{stem}.wallpaper.png"))
            }
        }
    }

    pub fn ensure_supported_format(&self, image: &Path) -> Result<PathBuf> {
        let extension = image.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if self.is_direct(&extension) {
            return Ok(image.to_path_buf());
        }

        let output = self.converted_path(image);
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if let (Some(converted), Some(original)) = (modified(&output), modified(image))
            && converted >= original
        {
            return Ok(output);
        }

        let file_name = image.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let unreadable = || ImageConversionError(file_name.clone());
        let decoded = image::ImageReader::open(image)
            .map_err(|_| unreadable())?
            .with_guessed_format()
            .map_err(|_| unreadable())?
            .decode()
            .map_err(|_| unreadable())?;

        let folder = output.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(folder)?;
        let temp = folder.join(format!(".{}.png", crate::json::unique_id()));
        let result = decoded
            .save_with_format(&temp, image::ImageFormat::Png)
            .map_err(|_| anyhow::Error::from(unreadable()))
            .and_then(|_| std::fs::rename(&temp, &output).map_err(Into::into));
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result.map(|_| output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_image(path: &Path, format: image::ImageFormat) {
        let img = image::RgbImage::from_pixel(4, 3, image::Rgb([0x7a, 0xa2, 0xf7]));
        image::DynamicImage::ImageRgb8(img).save_with_format(path, format).unwrap();
    }

    #[test]
    fn direct_formats_are_used_as_they_are() {
        let dir = tempfile::tempdir().unwrap();
        let jpg = dir.path().join("a.JPG");
        std::fs::write(&jpg, b"whatever").unwrap();

        assert_eq!(ImageConverter::new(ConvertedLayout::Mac).ensure_supported_format(&jpg).unwrap(), jpg);
        let bmp = dir.path().join("b.bmp");
        std::fs::write(&bmp, b"whatever").unwrap();
        assert_eq!(ImageConverter::new(ConvertedLayout::Windows).ensure_supported_format(&bmp).unwrap(), bmp);
    }

    #[test]
    fn webp_is_converted_to_png_beside_it_like_the_mac_app() {
        let dir = tempfile::tempdir().unwrap();
        let webp = dir.path().join("1.webp");
        write_image(&webp, image::ImageFormat::WebP);

        let png = ImageConverter::new(ConvertedLayout::Mac).ensure_supported_format(&webp).unwrap();

        assert_eq!(png, dir.path().join(".converted/1.webp.png"));
        let decoded = image::open(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (4, 3));
        assert_eq!(std::fs::read_dir(dir.path().join(".converted")).unwrap().count(), 1);
    }

    #[test]
    fn webp_and_bmp_follow_each_apps_layout() {
        let dir = tempfile::tempdir().unwrap();
        let webp = dir.path().join("2.webp");
        write_image(&webp, image::ImageFormat::WebP);
        let bmp = dir.path().join("3.bmp");
        write_image(&bmp, image::ImageFormat::Bmp);

        let windows = ImageConverter::new(ConvertedLayout::Windows).ensure_supported_format(&webp).unwrap();
        let mac = ImageConverter::new(ConvertedLayout::Mac).ensure_supported_format(&bmp).unwrap();

        assert_eq!(windows, dir.path().join(".2.wallpaper.png"));
        assert_eq!(mac, dir.path().join(".converted/3.bmp.png"));
        assert!(image::open(windows).is_ok() && image::open(mac).is_ok());
    }

    #[test]
    fn a_converted_copy_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let webp = dir.path().join("1.webp");
        write_image(&webp, image::ImageFormat::WebP);
        let converter = ImageConverter::new(ConvertedLayout::Mac);
        let png = converter.ensure_supported_format(&webp).unwrap();
        std::fs::write(&png, b"marker").unwrap();

        assert_eq!(converter.ensure_supported_format(&webp).unwrap(), png);
        assert_eq!(std::fs::read(&png).unwrap(), b"marker");
    }

    #[test]
    fn unreadable_images_get_an_actionable_error() {
        let dir = tempfile::tempdir().unwrap();
        let webp = dir.path().join("broken.webp");
        std::fs::write(&webp, b"not an image").unwrap();

        let error = ImageConverter::new(ConvertedLayout::Mac).ensure_supported_format(&webp).err().unwrap();

        assert_eq!(error.to_string(), "broken.webp couldn't be read as an image. Try downloading the theme again.");
        assert!(!dir.path().join(".converted/broken.webp.png").exists());
    }
}
