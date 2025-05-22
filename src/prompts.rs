// Contains prompt strings for commit summaries.

pub fn prompt_en(from: &str, to: &str, project_name: &str, lang: &str, commits: &str) -> String {
    format!(
        r#"
IMPORTANT: The prompt is english, but the generated output should be in {lang} language

Overall summary: Summarize all changes in the Git history in short, concise bullet points by grouping similar changes and highlighting their main topics and functions.

- Breakdown by day: Provide a summary of the changes for each day in a single line, highlighting the most important changes and features.
- If ticket numbers (format: [letter code]-[number sequence]) appear in the commit, add them to the daily overview at the end. e.g. "[...] relates to CPT-2345 and DSG-23212"
- If the commits are from multiple projects, repeat the output for each project, separated by --- and two line breaks before and after
- If there are no commits, this does not need to be mentioned.
- Use markdown, preserve it in the output, including spaces
- if no changes for a day, do not include it in the Daily breakdown
- Dateformat is YYYY-MM-DD

Commit Data: 
{commits}

Example for the Git history from {from} to {to}:


## {project_name} - Timeframe: {from} - {to}

*Overall summary*: 
- [Summary of changes. Sorted by topic]
- *[Topic / Topic Headline]*
    - [Details, up to 4, depending on complexity, can also be further nested]

Daily breakdown:
- [*Date 1*]: [Changes on this day summarized]
- [*Date 2*]: [Changes on this day summarized]
"#,
        from = from,
        to = to,
        project_name = project_name,
        lang = lang,
        commits = commits
    )
}
