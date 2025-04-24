# whathaveidone

A terminal tool to summarize your Git commit history for daily standups, using AI (Gemini API).

## Features

- Summarizes Git commit history for one or more projects
- Groups changes by day and topic
- Supports multiple repositories
- Customizable summary prompt (see `prompt.txt`)
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

   - To use German for the AI summary, add the `--lang de` parameter:
     ```sh
     whathaveidone --lang de
     # or
     whid --lang de
     ```

   - You can also specify the interval for commit history by providing one of these arguments:
     - `24` (default)
     - `48`
     - `72`
     - `week`
     - `month`
     
     Example for 1 week in German:
     ```sh
     whathaveidone week --lang de
     ```

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