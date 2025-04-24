// use reqwest;
// use serde_json;

/// Sends the commit list and a summary prompt to Gemini, returns the summary text.
pub async fn fetch_gemini_commit_summary(commits: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = std::fs::read_to_string("prompt.txt").unwrap_or_else(|_| String::from("Summarize the following git history:"));
    let user_message = format!("{}\n\nGit-History:\n{}", prompt, commits);
    let response = gemini_rs::chat("gemini-2.0-flash")
        .send_message(&user_message)
        .await?;
    let text = response.candidates
        .get(0)
        .and_then(|c| c.content.parts.get(0))
        .and_then(|p| p.text.as_ref())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Keine Zusammenfassung erhalten.".to_string());
    Ok(text)
}