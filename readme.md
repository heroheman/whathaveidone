# whathaveidone

**A terminal tool to summarize your Git commit history for daily standups, using AI (Gemini API).**

## Features
- Summarizes Git commit history for one or more projects
- Groups changes by day and topic
- Supports multiple repositories
- Customizable summary prompt (see prompt.txt)
- Copy summary to clipboard with one keypress

## Installation
### Prerequisites
- Rust (for building)
- A Gemini API key

### Build & Install

Clone and build the app:
```
git clone https://github.com/yourname/standup
cd standup/standup
cargo build --release
```

Or install directly (if published):
```
cargo install --git https://github.com/heroheman/whathaveidone
```

## Usage
1. Set your Gemini API key:
`GEMINI_API_TOKEN = `
or 
`export GEMINI_API_KEY=your-key-here`

2. Run the app in your project directory:
`whathaveidone` or `whid`

or anywhere above for multiple git repos.

Set a timeframe with `whid today`

3. Use the keyboard to navigate:

Arrow keys / HJKL: Move between projects/commits
- `a`: Show AI summary popup
- `c`: Copy summary to clipboard
- `q`: Quit

## Customizing the AI Prompt
Edit the file prompt.txt to change how the summary is generated.
The default prompt includes instructions for grouping by day, topic, and handling ticket numbers.
