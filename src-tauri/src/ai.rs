//! Inline AI text via any OpenAI-compatible chat completions endpoint.

use crate::settings::Settings;
use anyhow::{anyhow, Result};

/// Ask the configured OpenAI-compatible endpoint to transform `text`
/// according to `instruction`. Runs synchronously (call from a worker thread).
pub fn complete(settings: &Settings, instruction: &str, text: &str) -> Result<String> {
    let key = settings.openai_api_key.trim();
    if key.is_empty() {
        return Err(anyhow!(
            "No API key configured. Open Settings and paste your OpenAI API key."
        ));
    }
    let base = settings.openai_base_url.trim().trim_end_matches('/');
    let url = format!("{base}/chat/completions");

    let body = serde_json::json!({
        "model": settings.openai_model,
        "messages": [
            {
                "role": "system",
                "content": "You are a concise inline writing assistant inside a note-taking app. Follow the user's instruction about the provided note text. Reply ONLY with the transformed text — no explanations, no quotes around the result."
            },
            {
                "role": "user",
                "content": format!("Instruction: {instruction}\n\nNote text:\n{text}")
            }
        ],
        "temperature": 0.7
    });

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {key}"))
        .set("Content-Type", "application/json")
        .send_json(body)?;

    let json: serde_json::Value = resp.into_json()?;
    let content = json
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match content {
        Some(text) => Ok(text),
        None => Err(anyhow!(
            "Unexpected API response: {}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        )),
    }
}
