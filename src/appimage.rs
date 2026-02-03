//! AppImage detection, extraction, and integration logic.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{debug, info};

/// ELF magic bytes
const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// AppImage Type 1 magic at offset 8: "AI\x01"
const APPIMAGE_TYPE1_MAGIC: [u8; 3] = [0x41, 0x49, 0x01];

/// AppImage Type 2 magic at offset 8: "AI\x02"
const APPIMAGE_TYPE2_MAGIC: [u8; 3] = [0x41, 0x49, 0x02];

#[derive(Error, Debug)]
pub enum AppImageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Not a valid AppImage: {0}")]
    NotAppImage(String),
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("No .desktop file found in AppImage")]
    NoDesktopFile,
    #[error("Failed to parse .desktop file: {0}")]
    DesktopParseError(String),
}

/// Represents an AppImage type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppImageType {
    Type1,
    Type2,
}

/// Information extracted from an AppImage
#[derive(Debug, Clone)]
pub struct AppImageInfo {
    pub path: PathBuf,
    pub appimage_type: AppImageType,
    pub desktop_file: Option<PathBuf>,
    pub icon_files: Vec<PathBuf>,
    pub name: Option<String>,
}

/// Check if a file is a valid AppImage by examining magic bytes
pub fn is_appimage(path: &Path) -> bool {
    // First check file extension as a quick filter
    let has_extension = path
        .extension()
        .map(|ext| {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            ext_lower == "appimage"
        })
        .unwrap_or(false);

    // If no AppImage extension, could still be valid - check magic bytes
    match check_magic_bytes(path) {
        Ok(Some(_)) => true,
        Ok(None) => {
            if has_extension {
                debug!(
                    "File has .AppImage extension but invalid magic bytes: {:?}",
                    path
                );
            }
            false
        }
        Err(e) => {
            debug!("Error checking magic bytes for {:?}: {}", path, e);
            false
        }
    }
}

/// Check magic bytes and return the AppImage type if valid
fn check_magic_bytes(path: &Path) -> Result<Option<AppImageType>, AppImageError> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 16];

    let bytes_read = file.read(&mut header)?;
    if bytes_read < 11 {
        return Ok(None);
    }

    // Check ELF magic
    if header[0..4] != ELF_MAGIC {
        return Ok(None);
    }

    // Check AppImage magic at offset 8
    if header[8..11] == APPIMAGE_TYPE1_MAGIC {
        return Ok(Some(AppImageType::Type1));
    }

    if header[8..11] == APPIMAGE_TYPE2_MAGIC {
        return Ok(Some(AppImageType::Type2));
    }

    Ok(None)
}

/// Get the AppImage type
pub fn get_appimage_type(path: &Path) -> Result<AppImageType, AppImageError> {
    check_magic_bytes(path)?.ok_or_else(|| AppImageError::NotAppImage(path.display().to_string()))
}

/// Make an AppImage executable
pub fn make_executable(path: &Path) -> Result<(), AppImageError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();

    // Add execute permission for user, group, and others (matching read permission)
    let new_mode = mode | ((mode & 0o444) >> 2);

    if new_mode != mode {
        permissions.set_mode(new_mode);
        fs::set_permissions(path, permissions)?;
        info!("Made executable: {:?}", path);
    }

    Ok(())
}

/// Extract metadata from an AppImage
///
/// Extracts .desktop and icon files to a temporary directory and returns info about them.
pub fn extract_metadata(path: &Path, extract_dir: &Path) -> Result<AppImageInfo, AppImageError> {
    let appimage_type = get_appimage_type(path)?;

    // Ensure the AppImage is executable
    make_executable(path)?;

    // Create extraction directory
    fs::create_dir_all(extract_dir)?;

    // Try selective extraction first (faster)
    let selective_ok = try_selective_extract(path, extract_dir);

    // If selective extraction fails, do full extraction
    if !selective_ok {
        debug!("Selective extraction failed, trying full extraction");
        full_extract(path, extract_dir)?;
    }

    // Find extracted files
    let (desktop_file, icon_files) = find_extracted_files(extract_dir)?;

    let name = desktop_file
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().to_string());

    Ok(AppImageInfo {
        path: path.to_path_buf(),
        appimage_type,
        desktop_file,
        icon_files,
        name,
    })
}

/// Try to selectively extract only .desktop and icon files
fn try_selective_extract(appimage_path: &Path, extract_dir: &Path) -> bool {
    // Try to extract .desktop files
    let desktop_result = Command::new(appimage_path)
        .arg("--appimage-extract")
        .arg("*.desktop")
        .current_dir(extract_dir)
        .output();

    let desktop_ok = desktop_result.map(|o| o.status.success()).unwrap_or(false);

    // Try to extract icons (various formats and locations)
    let icon_patterns = ["*.png", "*.svg", "*.xpm", "usr/share/icons/*", ".DirIcon"];

    for pattern in &icon_patterns {
        let _ = Command::new(appimage_path)
            .arg("--appimage-extract")
            .arg(pattern)
            .current_dir(extract_dir)
            .output();
    }

    desktop_ok
}

/// Do a full extraction of the AppImage
fn full_extract(appimage_path: &Path, extract_dir: &Path) -> Result<(), AppImageError> {
    let output = Command::new(appimage_path)
        .arg("--appimage-extract")
        .current_dir(extract_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppImageError::ExtractionFailed(stderr.to_string()));
    }

    Ok(())
}

/// Find .desktop and icon files in the extraction directory
fn find_extracted_files(
    extract_dir: &Path,
) -> Result<(Option<PathBuf>, Vec<PathBuf>), AppImageError> {
    let squashfs_root = extract_dir.join("squashfs-root");
    let search_dir = if squashfs_root.exists() {
        squashfs_root
    } else {
        extract_dir.to_path_buf()
    };

    let mut desktop_file = None;
    let mut icon_files = Vec::new();

    // Walk the directory tree
    if let Ok(entries) = walk_dir(&search_dir) {
        for path in entries {
            // Check for .DirIcon first (no extension)
            if path.file_name().map(|n| n == ".DirIcon").unwrap_or(false) {
                icon_files.push(path);
                continue;
            }

            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                match ext_lower.as_str() {
                    "desktop" => {
                        // Prefer .desktop files in the root of squashfs-root
                        if desktop_file.is_none() || path.parent() == Some(&search_dir) {
                            desktop_file = Some(path);
                        }
                    }
                    "png" | "svg" | "xpm" => {
                        icon_files.push(path);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok((desktop_file, icon_files))
}

/// Recursively walk a directory and collect all file paths
fn walk_dir(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_dir(&path)?);
            } else {
                files.push(path);
            }
        }
    }

    Ok(files)
}

/// Generate a unique identifier for an AppImage based on its path
pub fn generate_identifier(path: &Path) -> String {
    let digest = md5::compute(path.to_string_lossy().as_bytes());
    format!("{:x}", digest)
}

/// Get the best icon from a list of icon files
///
/// Prefers larger PNG icons, then SVG, then anything else
pub fn select_best_icon(icons: &[PathBuf]) -> Option<&PathBuf> {
    // First, try to find a good-sized PNG
    let mut best_png: Option<(&PathBuf, u32)> = None;

    for icon in icons {
        if let Some(ext) = icon.extension()
            && ext.to_string_lossy().to_lowercase() == "png"
        {
            // Try to extract size from path (e.g., 256x256)
            let size = extract_icon_size(icon).unwrap_or(0);
            match best_png {
                None => best_png = Some((icon, size)),
                Some((_, best_size)) if size > best_size => {
                    best_png = Some((icon, size));
                }
                _ => {}
            }
        }
    }

    if let Some((icon, _)) = best_png {
        return Some(icon);
    }

    // Fall back to SVG
    for icon in icons {
        if let Some(ext) = icon.extension()
            && ext.to_string_lossy().to_lowercase() == "svg"
        {
            return Some(icon);
        }
    }

    // Fall back to anything
    icons.first()
}

/// Try to extract icon size from path (e.g., "256x256" -> 256)
fn extract_icon_size(path: &Path) -> Option<u32> {
    let path_str = path.to_string_lossy();

    // Look for patterns like "256x256" or "128x128"
    for component in path_str.split('/') {
        if let Some(size_str) = component.split('x').next()
            && let Ok(size) = size_str.parse::<u32>()
        {
            return Some(size);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identifier() {
        let path = Path::new("/home/user/Downloads/test.AppImage");
        let id = generate_identifier(path);
        assert!(!id.is_empty());
        assert_eq!(id.len(), 32); // MD5 hex is 32 chars
    }

    #[test]
    fn test_extract_icon_size() {
        let path = Path::new("/usr/share/icons/hicolor/256x256/apps/test.png");
        assert_eq!(extract_icon_size(path), Some(256));

        let path = Path::new("/some/path/icon.png");
        assert_eq!(extract_icon_size(path), None);
    }
}
