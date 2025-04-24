// Contains prompt strings for commit summaries.

pub const PROMPT_EN: &str = r#"Overall summary: Summarize all changes in the Git history in short, concise bullet points by grouping similar changes and highlighting their main topics and functions.

- Breakdown by day: Provide a summary of the changes for each day in a single line, highlighting the most important changes and features.
- If ticket numbers (format: [letter code]-[number sequence]) appear in the commit, add them to the daily overview at the end. e.g. "[...] relates to CPT-2345 and DSG-23212"
- If the commits are from multiple projects, repeat the output for each project, separated by --- and two line breaks before and after
- If there are no commits, this does not need to be mentioned.
- Use markdown, preserve it in the output, including spaces

Example for the Git history from [date] to [date]:


## [PROJECT NAME from GIT] - Timeframe: [FROM] - [TO]

*Overall summary*: 
- [Summary of changes. Sorted by topic]
- *[Topic / Topic Headline]*
    - [Details, up to 4, depending on complexity, can also be further nested]

Daily breakdown:
- [*Date 1*]: [Changes on this day summarized]
- [*Date 2*]: [Changes on this day summarized]"#;

pub const PROMPT_DE: &str = r#"Gesamtübersicht: Fasse alle Änderungen in der Git-Historie in kurzen, prägnanten Stichpunkten zusammen, indem ähnliche Änderungen gruppiert und deren Hauptthemen und Funktionen hervorgehoben werden.

- Aufschlüsselung nach Tag: Gib für jeden Tag eine Zusammenfassung der Änderungen in einer einzigen Zeile an und hebe die wichtigsten Änderungen und Features hervor.
- Falls Ticketnummern (Format: [Buchstabencode]-[Zahlenfolge]) im Commit erscheinen, füge sie am Ende der Tagesübersicht hinzu, z. B. "[...] bezieht sich auf CPT-2345 und DSG-23212"
- Wenn die Commits aus mehreren Projekten stammen, wiederhole die Ausgabe für jedes Projekt, getrennt durch --- und jeweils zwei Zeilenumbrüche davor und danach
- Wenn es keine Commits gibt, muss dies nicht erwähnt werden.
- Verwende Markdown und erhalte es im Output, inklusive Leerzeichen

Beispiel für die Git-Historie von [Datum] bis [Datum]:


## [PROJEKTNAME aus GIT] - Zeitraum: [VON] - [BIS]

*Gesamtübersicht*: 
- [Zusammenfassung der Änderungen. Nach Themen sortiert]
- *[Thema / Themenüberschrift]*
    - [Details, bis zu 4, je nach Komplexität, ggf. weiter verschachtelt]

Tagesübersicht:
- [*Datum 1*]: [Änderungen an diesem Tag zusammengefasst]
- [*Datum 2*]: [Änderungen an diesem Tag zusammengefasst]"#;
