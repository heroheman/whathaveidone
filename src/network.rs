// use reqwest;
// use serde_json;

/// Sends the commit list and a summary prompt to Gemini, returns the summary text.
pub async fn fetch_gemini_commit_summary(commits: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prompt = r#"Gesamtzusammenfassung: Fasse alle Änderungen in der Git-Historie in kurzen, prägnanten Stichpunkten zusammen, indem du ähnliche Änderungen gruppierst und deren Hauptthemen und Funktionen hervorhebst.

- Aufschlüsselung nach Tagen: Gib eine Zusammenfassung der Änderungen für jeden Tag in einer einzelnen Zeile, und hebe jeweils die wichtigsten Änderungen und Funktionen hervor.
- Wenn sich Ticketnummern (Format: [Buchstabenkürzel]-[Zahlenfolge]) im Commit befinden, füge diese auch zur Tagesübersicht am Ende hinzu. z.B. "[...] betrifft CPT-2345 und DSG-23212
- Falls die Commits aus mehreren Projekte mitgeschickt werden, wiederhole die Ausgabe für jedes Projekt, getrennt durch ein --- und zwei Umbrueche davor und danach
- wenn es keine commits gibt, muss dieses nicht erwähnt werden. 
- nutze markdown, erhalte dies auch in der ausgabe, inkl spaces

Beispiel für die Git-Historie von [Datum] bis [Datum]:
## [PROJEKTNAME aus GIT] - Zeitfenster: [VON] - [BIS]

*Gesamtzusammenfassung*: 
- [Zusammenfassung der Änderungen. Sortiert nach Themenbereiche]
- *[Thema / Topic Headline]*
    - [Detailausgaben, bis zu 4, je nach Komplexität, kann auch weiter verschachtelt werden]

Tägliche Aufschlüsselung:
- [*Datum 1*]: [Änderungen an diesem Tag zusammengefasst]
- [*Datum 2*]: [Änderungen an diesem Tag zusammengefasst]
"#;
    let user_message = format!("{}\n\nGit-Historie:\n{}", prompt, commits);
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