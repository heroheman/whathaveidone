use reqwest;
use serde_json;

pub async fn fetch_quote() -> Result<String, reqwest::Error> {
    let resp = reqwest::get("https://dummyjson.com/quotes/random").await?;
    let json: serde_json::Value = resp.json().await?;
    let quote = json.get("quote").and_then(|q| q.as_str()).unwrap_or("Kein Zitat gefunden.");
    let author = json.get("author").and_then(|a| a.as_str()).unwrap_or("");
    Ok(format!("{}\n\n— {}", quote, author))
}

// Gemini Star Trek quote fetcher
pub async fn fetch_gemini_startrek_quote() -> Result<String, Box<dyn std::error::Error>> {
    let response = gemini_rs::chat("gemini-2.0-flash")
        .send_message("Give me a random Star Trek quote.")
        .await?;
    let text = response.candidates
        .get(0)
        .and_then(|c| c.content.parts.get(0))
        .and_then(|p| p.text.as_ref())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Kein Zitat gefunden.".to_string());
    Ok(text)
}