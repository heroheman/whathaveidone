# whathaveidone

A terminal tool to summarize your Git commit history for daily standups, using AI (Gemini API).

<a href="https://asciinema.org/a/l58gl6wettdA3x4eLD4jCkWkq" target="_blank"><img src="https://asciinema.org/a/l58gl6wettdA3x4eLD4jCkWkq.svg" /></a>

## Features

- Summarizes Git commit history for one or more projects
- Groups changes by day and topic
- Supports multiple repositories
- Customizable summary prompt (see below for custom prompt usage)
- Copy summary to clipboard with one keypress
- **Mark commits with m, view all marked with S**

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (for building)
- A [Gemini API key](https://aistudio.google.com/app/apikey)

### Build & Install

- `cargo install whathaveidone`


## Usage

1. Set your Gemini API key:
   ```sh
   export GEMINI_API_KEY=your-key-here
   ```

2. Run the app in your commandline with `whathaveidone` or just `whid`. It will look relative from the folder you started the app. 

   - To use a specific language for the AI summary, add the `--lang <language>` parameter (e.g. `--lang german` for German, `--lang english` for English). If not specified, the default is `english`.
   - **Note**: the translation is made by gemini itself
     ```sh
     whathaveidone --lang de
     # or
     whid --lang english
     ```

   - You can also specify the interval for commit history by providing one of these arguments:
     - `24` or `today` (default)
     - `48`
     - `72` or `yesterday`
     - `week`
     - `month`
     
     Example for 1 week in German:
     ```sh
     whathaveidone week --lang de
     ```

   - **Custom prompt:**
     You can provide a custom prompt template file using the `--prompt <filename.txt>` option (relative or absolute path). If this option is used, the file content will be sent as the prompt to the AI (no default wrapping or formatting). You can use the following placeholders in your template, which will be replaced accordingly:

     - `{from}`: Start date of the selected interval (format: YYYY-MM-DD)
     - `{to}`: End date of the selected interval (format: YYYY-MM-DD)
     - `{project}`: Name of the selected project
     - `{projectname}`: Alias for `{project}`
     - `{interval}`: The selected interval label (e.g. "week", "24h")
     - `{lang}`: The language parameter (e.g. "de", "english")
     - `{commits}`: The commit data to be summarized

     Example:
     ```sh
     whathaveidone --prompt myprompt.txt
     ```
     If the file cannot be loaded, the default prompt will be used.

3. Use the keyboard to navigate:
   - Arrow keys: Move between projects/commits
   - `a`: Show AI summary popup
   - `c`: Copy summary to clipboard
   - `m`: Mark/unmark commit
   - `s`: Show popup with all marked commits
   - `A`: Show AI summary popup
   - `Q`: Quit


## Links
- [Whathaveidone](https://crates.io/crates/whathaveidone) on Crates
- [Whathaveidone](https://github.com/heroheman/whathaveidone) on Github