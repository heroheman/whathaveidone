# whathaveidone

A terminal tool to summarize your Git commit history for daily standups, using AI (Gemini API).

<a href="https://asciinema.org/a/l58gl6wettdA3x4eLD4jCkWkq" target="_blank"><img src="https://asciinema.org/a/l58gl6wettdA3x4eLD4jCkWkq.svg" /></a>

---

## Features
- Summarizes Git commit history for one or more projects
- Groups changes by day and topic
- Supports multiple repositories
- Customizable summary prompt
- Copy summary to clipboard with one keypress
- Mark commits with `m`, view all marked with `s`

---

## Installation

### Prerequisites
- [Rust](https://rustup.rs/) (for building)
- A [Gemini API key](https://aistudio.google.com/app/apikey)

### Build & Install
```sh
cargo install whathaveidone
```

### Set up your Gemini API key
You must set your Gemini API key before running the app:
```sh
export GEMINI_API_KEY=your-key-here
```
Add this line to your shell profile (e.g. `~/.zshrc`) to make it persistent across terminal sessions.

---

## Usage

Run the app in your terminal:
```sh
whathaveidone
# or
whid
```

### Gemini model selection
You can select the Gemini model version with the `--gemini <model>` parameter. The default is `gemini-2.0-flash`.

Example:
```sh
whathaveidone --gemini gemini-1.5-pro
#or 
whathaveidone --gemini gemini-2.5-flash-preview-05-20

```
The selected model will be shown in the summary popup while waiting for the AI response. 

### Language selection
To use a specific language for the AI summary, add the `--lang <language>` parameter:
```sh
whathaveidone --lang german      # German
whid --lang english              # English (default)
```
**Note:** The translation is performed by Gemini itself.

### Time interval selection
Use `TAB` or `SHIFT-TAB` for interval selection. 

Alternativly: You can specify the start interval for commit history as parameter:
- `24` or `today` (default)
- `48`
- `72` or `yesterday`
- `week`
- `month`

Example for 1 week in German:
```sh
whathaveidone yesterday
# or
whathaveidone 48
# or
whathaveidone week --lang german
```

### Custom prompt
You can provide a custom prompt template file using the `--prompt <filename.txt>` option. Placeholders in your template will be replaced automatically:
- `{from}`: Start date (YYYY-MM-DD)
- `{to}`: End date (YYYY-MM-DD)
- `{project}` or `{projectname}`: Project name
- `{interval}`: Interval label (e.g. "week")
- `{lang}`: Language (e.g. "german", "english")
- `{commits}`: Commit data to be summarized

Example:
```sh
whathaveidone --prompt myprompt.txt
```
If the file cannot be loaded, the default prompt will be used.

---

## Keyboard Shortcuts
- Arrow keys: Move between projects/commits
- `a` or `A`: Show AI summary popup
- `c`: Copy summary to clipboard
- `m`: Mark/unmark commit
- `s`: Show popup with all marked commits
- `Q`: Quit

---

## Links
- [whathaveidone on Crates.io](https://crates.io/crates/whathaveidone)
- [whathaveidone on GitHub](https://github.com/heroheman/whathaveidone)