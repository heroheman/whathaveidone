use reqwest;
use serde_json;

pub async fn fetch_quote() -> Result<String, reqwest::Error> {
    let resp = reqwest::get("https://dummyjson.com/quotes/random").await?;
    let json: serde_json::Value = resp.json().await?;
    let quote = json.get("quote").and_then(|q| q.as_str()).unwrap_or("Kein Zitat gefunden.");
    let author = json.get("author").and_then(|a| a.as_str()).unwrap_or("");
    Ok(format!("{}\n\n— {}", quote, author))
}