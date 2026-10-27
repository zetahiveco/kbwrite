//! Local MCP server for the open vault.
//!
//! Settings starts and stops it. Claude, Codex, and OpenCode connect to
//! `http://127.0.0.1:47321/mcp` while KbWrite is running.

use crate::commands::{self, VaultSlot};
use crate::settings::Settings;
use crate::vault::Vault;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const PORT: u16 = 47321;

const PROTOCOL: &str = "2025-03-26";
const ACCEPTED: &[&str] = &[
    "2024-11-05",
    "2025-03-26",
    "2025-06-18",
    "2025-11-25",
];

pub struct Deps {
    pub settings: Arc<Mutex<Settings>>,
    pub vault: Arc<Mutex<VaultSlot>>,
    pub on_change: Arc<dyn Fn() + Send + Sync>,
}

pub struct Running {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Running {
    pub fn alive(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn shutdown(self) {}
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub running: bool,
    pub port: u16,
    pub url: String,
    pub claude: String,
    pub codex: String,
    pub opencode: String,
}

pub fn status(running: bool) -> Status {
    Status {
        running,
        port: PORT,
        url: format!("http://127.0.0.1:{PORT}/mcp"),
        claude: format!("claude mcp add --transport http kbwrite http://127.0.0.1:{PORT}/mcp"),
        codex: format!("codex mcp add kbwrite --url http://127.0.0.1:{PORT}/mcp"),
        opencode: format!("opencode mcp add kbwrite --url http://127.0.0.1:{PORT}/mcp"),
    }
}

pub fn serve(deps: Deps) -> Result<Running, String> {
    let listener = TcpListener::bind(("127.0.0.1", PORT)).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            format!("Port {PORT} is already in use.")
        } else {
            format!("Could not start the MCP server: {error}")
        }
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Could not start the MCP server: {error}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let thread = thread::spawn(move || accept_loop(listener, flag, deps));
    Ok(Running {
        stop,
        thread: Some(thread),
    })
}

fn accept_loop(listener: TcpListener, stop: Arc<AtomicBool>, deps: Deps) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
                let _ = handle_client(&mut stream, &deps);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(40));
            }
            Err(_) => break,
        }
    }
}

fn handle_client(stream: &mut TcpStream, deps: &Deps) -> std::io::Result<()> {
    let request = match read_request(stream) {
        Ok(request) => request,
        Err(_) => {
            write_raw(stream, 400, "application/json", b"{\"error\":\"bad request\"}")?;
            return Ok(());
        }
    };
    if request.path != "/mcp" && request.path != "/mcp/" {
        write_raw(stream, 404, "text/plain", b"not found")?;
        return Ok(());
    }
    if !host_allowed(request.header("host")) || !origin_allowed(request.header("origin")) {
        write_raw(stream, 403, "text/plain", b"forbidden")?;
        return Ok(());
    }
    if request.method == "OPTIONS" {
        write_raw(stream, 204, "text/plain", b"")?;
        return Ok(());
    }
    if request.method == "GET" || request.method == "DELETE" {
        write_raw(stream, 405, "application/json", b"{\"error\":\"use POST\"}")?;
        return Ok(());
    }
    if request.method != "POST" {
        write_raw(stream, 405, "application/json", b"{\"error\":\"use POST\"}")?;
        return Ok(());
    }
    let accept = request.header("accept").unwrap_or("");
    if !accept.is_empty() && !accept.contains("application/json") && !accept.contains("text/event-stream")
    {
        write_raw(stream, 406, "application/json", b"{\"error\":\"not acceptable\"}")?;
        return Ok(());
    }
    let body = String::from_utf8_lossy(&request.body);
    let Some(reply) = dispatch(body.trim(), deps) else {
        write_raw(stream, 202, "application/json", b"")?;
        return Ok(());
    };
    let bytes = reply.into_bytes();
    if accept.contains("text/event-stream") && !accept.contains("application/json") {
        let event = format!("event: message\ndata: {}\n\n", String::from_utf8_lossy(&bytes));
        write_raw(stream, 200, "text/event-stream", event.as_bytes())?;
    } else {
        write_raw(stream, 200, "application/json", &bytes)?;
    }
    Ok(())
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut header_bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if stream.read(&mut byte)? == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "closed"));
        }
        header_bytes.push(byte[0]);
        if header_bytes.len() > 64 * 1024 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "headers"));
        }
        if header_bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&header_bytes);
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    if length > 8 * 1024 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "body"));
    }
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body)?;
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn write_raw(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: http://127.0.0.1\r\nAccess-Control-Allow-Headers: Content-Type, Accept, Mcp-Session-Id, MCP-Protocol-Version\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn host_allowed(host: Option<&str>) -> bool {
    let Some(host) = host else {
        return true;
    };
    let host = host.trim().trim_start_matches('[');
    host.starts_with("127.0.0.1") || host.starts_with("localhost") || host.starts_with("::1")
}

fn origin_allowed(origin: Option<&str>) -> bool {
    let Some(origin) = origin.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if origin == "null" {
        return true;
    }
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or("").trim_start_matches('[');
    let host = host.split([':', ']']).next().unwrap_or("");
    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

/// A JSON-RPC response, or `None` when the message was a notification.
fn dispatch(body: &str, deps: &Deps) -> Option<String> {
    let message: Value = match serde_json::from_str(body) {
        Ok(Value::Object(_)) => serde_json::from_str(body).ok()?,
        Ok(_) => {
            return Some(rpc_error(Value::Null, -32600, "Expected a JSON-RPC object"));
        }
        Err(_) => return Some(rpc_error(Value::Null, -32700, "Parse error")),
    };
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    if id.is_none() {
        return None;
    }
    let id = id.unwrap_or(Value::Null);
    let result = match method {
        "initialize" => initialize(&message),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => match call_tool(&message, deps) {
            Ok(value) => value,
            Err(error) => {
                return Some(rpc_error(id, -32602, &error));
            }
        },
        "resources/list" => json!({ "resources": [] }),
        "prompts/list" => json!({ "prompts": [] }),
        "logging/setLevel" => json!({}),
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(rpc_ok(id, result))
}

fn initialize(message: &Value) -> Value {
    let requested = message
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL);
    let version = if ACCEPTED.contains(&requested) {
        requested
    } else {
        PROTOCOL
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "KbWrite", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "KbWrite notes are markdown files in the open vault. Use list_notes, read_note, search_notes, create_note, write_note, and delete_note. Paths are relative to the vault, like Welcome.md."
    })
}

fn tools() -> Vec<Value> {
    vec![
        tool(
            "list_notes",
            "List the markdown notes in the KbWrite vault.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "read_note",
            "Read one note. `path` is a vault-relative path or a note title.",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        ),
        tool(
            "search_notes",
            "Find notes whose title or body contains the query.",
            json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }),
        ),
        tool(
            "create_note",
            "Create a new markdown note in the vault root.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["title"]
            }),
        ),
        tool(
            "write_note",
            "Replace a note's markdown. Creates the file when it does not exist.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            "delete_note",
            "Delete one markdown note from the vault.",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        ),
    ]
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": schema
    })
}

fn call_tool(message: &Value, deps: &Deps) -> Result<Value, String> {
    let name = message
        .pointer("/params/name")
        .and_then(Value::as_str)
        .ok_or("Tool name is required")?;
    let args = message.pointer("/params/arguments").cloned().unwrap_or_else(|| json!({}));
    let (text, changed) = match name {
        "list_notes" => (list_notes(deps)?, false),
        "read_note" => (read_note(deps, arg(&args, "path")?)?, false),
        "search_notes" => (search_notes(deps, arg(&args, "query")?)?, false),
        "create_note" => (
            create_note(deps, arg(&args, "title")?, optional(&args, "content"))?,
            true,
        ),
        "write_note" => (
            write_note(deps, arg(&args, "path")?, arg(&args, "content")?)?,
            true,
        ),
        "delete_note" => (delete_note(deps, arg(&args, "path")?)?, true),
        _ => return Err(format!("Unknown tool {name}")),
    };
    if changed {
        (deps.on_change)();
    }
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    }))
}

fn arg(args: &Value, name: &str) -> Result<String, String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{name} is required"))
}

fn optional(args: &Value, name: &str) -> String {
    args.get(name).and_then(Value::as_str).unwrap_or("").to_string()
}

fn with_vault<T>(
    deps: &Deps,
    f: impl FnOnce(&Path, &mut Vault) -> Result<T, String>,
) -> Result<T, String> {
    let settings = deps.settings.lock().map_err(|error| error.to_string())?;
    let mut slot = deps.vault.lock().map_err(|error| error.to_string())?;
    let root = settings.vault_path.clone();
    let missing = slot
        .error
        .clone()
        .unwrap_or_else(|| "No vault is open".to_string());
    let vault = slot.vault.as_mut().ok_or(missing)?;
    f(&root, vault)
}

fn list_notes(deps: &Deps) -> Result<String, String> {
    with_vault(deps, |_, vault| {
        let (notes, _) = vault.load();
        if notes.is_empty() {
            return Ok("No notes.".to_string());
        }
        Ok(notes
            .iter()
            .map(|note| format!("{} — {}", note.rel, note.title))
            .collect::<Vec<_>>()
            .join("\n"))
    })
}

fn read_note(deps: &Deps, path: String) -> Result<String, String> {
    with_vault(deps, |root, vault| {
        let path = find_note(root, vault, &path)?;
        vault.read_note(&path).map_err(|error| format!("{error:#}"))
    })
}

fn search_notes(deps: &Deps, query: String) -> Result<String, String> {
    with_vault(deps, |_, vault| {
        let needle = query.to_lowercase();
        let (notes, _) = vault.load();
        let mut hits = Vec::new();
        for note in notes {
            let body = vault.read_note(&note.path).unwrap_or_default();
            let in_name = note.rel.to_lowercase().contains(&needle) || note.title.to_lowercase().contains(&needle);
            let in_body = body.to_lowercase().contains(&needle);
            if !in_name && !in_body {
                continue;
            }
            hits.push(format!("{}\n{}", note.rel, excerpt(&body, &needle)));
            if hits.len() == 20 {
                break;
            }
        }
        if hits.is_empty() {
            Ok(format!("No notes match “{query}”."))
        } else {
            Ok(hits.join("\n\n"))
        }
    })
}

fn create_note(deps: &Deps, title: String, content: String) -> Result<String, String> {
    with_vault(deps, |_, vault| {
        let note = vault
            .create_note(&title, &content)
            .map_err(|error| format!("{error:#}"))?;
        Ok(format!("Created {}", note.rel))
    })
}

fn write_note(deps: &Deps, path: String, content: String) -> Result<String, String> {
    with_vault(deps, |root, vault| {
        let path = if Path::new(&path).is_absolute() {
            let path = note_path(root, &path)?;
            commands::ensure_inside(root, &path)?;
            path
        } else {
            prepare_note(root, &path)?
        };
        if path.extension().map(|ext| ext != "md").unwrap_or(true) {
            return Err("Only markdown notes can be written".into());
        }
        vault
            .write_note(&path, &content)
            .map_err(|error| format!("{error:#}"))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        Ok(format!("Saved {rel}"))
    })
}

fn delete_note(deps: &Deps, path: String) -> Result<String, String> {
    with_vault(deps, |root, vault| {
        let path = find_note(root, vault, &path)?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        vault
            .delete_path(&path)
            .map_err(|error| format!("{error:#}"))?;
        let (notes, _) = vault.load();
        if notes.is_empty() {
            vault
                .create_note("Welcome", commands::WELCOME)
                .map_err(|error| format!("{error:#}"))?;
        }
        Ok(format!("Deleted {rel}"))
    })
}

fn find_note(root: &Path, vault: &mut Vault, raw: &str) -> Result<PathBuf, String> {
    let direct = note_path(root, raw)?;
    if direct.exists() {
        commands::ensure_inside(root, &direct)?;
        return Ok(direct);
    }
    let (notes, _) = vault.load();
    let needle = raw.trim().trim_end_matches(".md").to_lowercase();
    let raw_lower = raw.trim().to_lowercase();
    notes
        .into_iter()
        .find(|note| {
            note.title.to_lowercase() == needle
                || note.rel.to_lowercase() == raw_lower
                || note.rel.to_lowercase() == format!("{needle}.md")
        })
        .map(|note| note.path)
        .ok_or_else(|| format!("No note named {raw}"))
}

/// Build a vault-relative note path, creating missing folders only after each
/// existing prefix is confirmed to stay inside the vault.
fn prepare_note(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let path = note_path(root, raw)?;
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "Refusing to operate outside of the vault".to_string())?;
    let parts: Vec<_> = relative.components().collect();
    let mut cursor = root.to_path_buf();
    for (index, component) in parts.iter().enumerate() {
        let Component::Normal(part) = component else {
            return Err("Refusing to operate outside of the vault".into());
        };
        cursor.push(part);
        let is_file = index + 1 == parts.len();
        if cursor.exists() {
            commands::ensure_inside(root, &cursor)?;
        } else if !is_file {
            std::fs::create_dir(&cursor).map_err(|error| error.to_string())?;
        }
    }
    Ok(path)
}

fn note_path(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("A note path is required".into());
    }
    let incoming = Path::new(raw);
    if incoming.is_absolute() {
        commands::ensure_inside(root, incoming)?;
        return Ok(incoming.to_path_buf());
    }
    if incoming
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("Refusing to operate outside of the vault".into());
    }
    let mut path = root.to_path_buf();
    for component in incoming.components() {
        if let Component::Normal(part) = component {
            path.push(part);
        }
    }
    if path.extension().map(|ext| ext != "md").unwrap_or(true) {
        path.set_extension("md");
    }
    Ok(path)
}

fn excerpt(body: &str, needle: &str) -> String {
    let lower = body.to_lowercase();
    let Some(index) = lower.find(needle) else {
        let line = body.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
        return clip(line, 140);
    };
    let start = index.saturating_sub(40);
    clip(&body[start..], 160)
}

fn clip(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut snippet = text[..end].replace('\n', " ");
    if text.len() > end {
        snippet.push('…');
    }
    snippet
}

fn rpc_ok(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn rpc_error(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;

    fn fixture() -> (Deps, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "kbwrite-mcp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let vault = Vault::open(&dir).unwrap();
        let deps = Deps {
            settings: Arc::new(Mutex::new(Settings {
                vault_path: dir.clone(),
                dark_mode: true,
                ..Settings::default()
            })),
            vault: Arc::new(Mutex::new(VaultSlot {
                vault: Some(vault),
                error: None,
            })),
            on_change: Arc::new(|| {}),
        };
        (deps, dir)
    }

    #[test]
    fn tools_round_trip_and_refuse_escape() {
        let (deps, dir) = fixture();
        let created = call_tool(
            &json!({
                "params": {
                    "name": "create_note",
                    "arguments": { "title": "Plan", "content": "Ship the MCP server" }
                }
            }),
            &deps,
        )
        .unwrap();
        assert!(created["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Plan.md"));

        let found = call_tool(
            &json!({
                "params": { "name": "search_notes", "arguments": { "query": "Ship" } }
            }),
            &deps,
        )
        .unwrap();
        assert!(found["content"][0]["text"].as_str().unwrap().contains("Plan.md"));

        let escaped = call_tool(
            &json!({
                "params": { "name": "read_note", "arguments": { "path": "../secret" } }
            }),
            &deps,
        );
        assert!(escaped.is_err());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn initialize_echoes_a_supported_version() {
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "2024-11-05" }
        });
        let (deps, dir) = fixture();
        let reply: Value = serde_json::from_str(&dispatch(&message.to_string(), &deps).unwrap()).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2024-11-05");
        assert!(reply["result"]["capabilities"]["tools"].is_object());
        let _ = std::fs::remove_dir_all(dir);
    }
}
