//! Vault media files. Notes store `kbwrite-media:///` URLs. The `vault`
//! protocol serves those files back to the editor, and only from this vault.

use crate::commands::{self, AppState};
use base64::Engine;
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

const MAX_BYTES: usize = 80 * 1024 * 1024;
const MAX_TEXT: usize = 2 * 1024 * 1024;

const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "heic", "mp4", "webm", "mov",
    "ogv", "m4v", "mkv", "mp3", "wav", "ogg", "m4a", "aac", "flac", "pdf", "csv", "docx", "xlsx",
];

pub fn save(
    state: &AppState,
    note_path: &str,
    name: &str,
    data_base64: &str,
) -> Result<String, String> {
    let settings = state.settings.lock().map_err(|error| error.to_string())?;
    let root = settings.vault_path.clone();
    drop(settings);
    let note = PathBuf::from(note_path);
    commands::ensure_inside(&root, &note)?;
    if note.extension().map(|ext| ext != "md").unwrap_or(true) {
        return Err("Open a note before adding media".into());
    }
    let file_name = clean_media_name(name)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.trim())
        .map_err(|_| "Could not read that file".to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Files must be 80 MB or smaller".into());
    }
    let dir = root.join(".attachments");
    std::fs::create_dir_all(&dir).map_err(|error| format!("Could not store the file: {error}"))?;
    let mut dest = dir.join(&file_name);
    let stem = Path::new(&file_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = Path::new(&file_name)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut n = 2;
    while dest.exists() {
        dest = dir.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    commands::ensure_inside(&root, &dest)?;
    std::fs::write(&dest, &bytes).map_err(|error| format!("Could not store the file: {error}"))?;
    let stored = file_name_of(&dest);
    Ok(media_url(&format!(".attachments/{stored}")))
}

pub fn read_text(state: &AppState, url: &str) -> Result<String, String> {
    let path = locate(state, url)?;
    let bytes = std::fs::read(&path).map_err(|error| format!("Could not read the file: {error}"))?;
    if bytes.len() > MAX_TEXT {
        return Err("That file is too large to preview".into());
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub fn open(state: &AppState, url: &str) -> Result<(), String> {
    let path = locate(state, url)?;
    open_with_system(&path)
}

pub fn respond<R: Runtime>(
    app: &AppHandle<R>,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let relative = request
        .uri()
        .path()
        .trim_start_matches('/')
        .to_string();
    let state = app.state::<AppState>();
    let path = match locate(&state, &media_url(&percent_decode(&relative))) {
        Ok(path) => path,
        Err(error) => return plain(404, error),
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return plain(404, error.to_string()),
    };
    let mime = mime_for(&path);
    let range = request
        .headers()
        .get(tauri::http::header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    file_response(bytes, mime, range.as_deref())
}

fn locate(state: &AppState, url: &str) -> Result<PathBuf, String> {
    let settings = state.settings.lock().map_err(|error| error.to_string())?;
    let root = settings.vault_path.clone();
    drop(settings);
    let relative = vault_relative(url)?;
    let mut path = root.clone();
    for component in Path::new(&relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            _ => return Err("Refusing to open a file outside the vault".into()),
        }
    }
    commands::ensure_inside(&root, &path)?;
    if !path.is_file() {
        return Err("Media file not found".into());
    }
    Ok(path)
}

fn vault_relative(url: &str) -> Result<String, String> {
    let url = url.trim();
    let relative = url
        .strip_prefix("kbwrite-media:///")
        .or_else(|| url.strip_prefix("kbwrite-media://"))
        .unwrap_or(url)
        .trim_start_matches('/');
    let relative = percent_decode(relative);
    if relative.is_empty()
        || relative.contains('\\')
        || relative.split('/').any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Media path is invalid".into());
    }
    Ok(relative)
}

fn media_url(relative: &str) -> String {
    format!(
        "kbwrite-media:///{}",
        relative.trim_start_matches('/').replace('\\', "/")
    )
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

pub fn clean_media_name(name: &str) -> Result<String, String> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err("Choose a file with a name".into());
    }
    let base = name.trim();
    if base.is_empty() || base == "." || base == ".." || base.starts_with('.') {
        return Err("Choose a file with a name".into());
    }
    let ext = Path::new(base)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if !EXTENSIONS.contains(&ext.as_str()) {
        return Err(
            "Use an image, PDF, CSV, DOCX, XLSX, video, or audio file".into(),
        );
    }
    let stem = Path::new(base)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let mut safe = String::new();
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            safe.push(ch);
        } else {
            safe.push('-');
        }
    }
    let safe = safe.trim().trim_matches('-').to_string();
    if safe.is_empty() {
        return Err("Choose a file with a name".into());
    }
    Ok(format!("{safe}.{ext}"))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| value.to_string())
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "heic" => "image/heic",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "ogv" => "video/ogg",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" | "aac" => "audio/mp4",
        "flac" => "audio/flac",
        "pdf" => "application/pdf",
        "csv" => "text/csv; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn file_response(bytes: Vec<u8>, mime: &str, range: Option<&str>) -> tauri::http::Response<Vec<u8>> {
    let total = bytes.len();
    if let Some((start, end)) = parse_range(range, total) {
        let slice = bytes[start..=end].to_vec();
        let len = slice.len();
        return tauri::http::Response::builder()
            .status(206)
            .header(tauri::http::header::CONTENT_TYPE, mime)
            .header(tauri::http::header::ACCEPT_RANGES, "bytes")
            .header(
                tauri::http::header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{total}"),
            )
            .header(tauri::http::header::CONTENT_LENGTH, len)
            .header(tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(slice)
            .unwrap_or_else(|_| plain(500, "Could not read the file".into()));
    }
    let len = bytes.len();
    tauri::http::Response::builder()
        .status(200)
        .header(tauri::http::header::CONTENT_TYPE, mime)
        .header(tauri::http::header::ACCEPT_RANGES, "bytes")
        .header(tauri::http::header::CONTENT_LENGTH, len)
        .header(tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(bytes)
        .unwrap_or_else(|_| plain(500, "Could not read the file".into()))
}

fn parse_range(header: Option<&str>, total: usize) -> Option<(usize, usize)> {
    if total == 0 {
        return None;
    }
    let spec = header?.strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let start: usize = start.parse().ok()?;
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse().unwrap_or(total - 1).min(total - 1)
    };
    if start > end || start >= total {
        return None;
    }
    Some((start, end))
}

fn plain(status: u16, message: String) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(status)
        .header(tauri::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(message.into_bytes())
        .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

fn open_with_system(path: &Path) -> Result<(), String> {
    let path = path.to_string_lossy().to_string();
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(&path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", &path]);
        command
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(&path);
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open the file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_documents_and_rejects_executables() {
        assert_eq!(
            clean_media_name("Quarterly Report.xlsx").unwrap(),
            "Quarterly Report.xlsx"
        );
        assert!(clean_media_name("../secret.pdf").is_err());
        assert!(clean_media_name("notes.exe").is_err());
        assert!(clean_media_name(".hidden.png").is_err());
    }

    #[test]
    fn media_urls_stay_inside_the_vault_path() {
        assert_eq!(
            vault_relative("kbwrite-media:///.attachments/photo.png").unwrap(),
            ".attachments/photo.png"
        );
        assert!(vault_relative("kbwrite-media:///../secret.pdf").is_err());
    }
}
