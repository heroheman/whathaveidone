# whathaveidone

A terminal tool to summarize your Git commit history for daily standups, using AI (Gemini API).

## Features

- Summarizes Git commit history for one or more projects
- Groups changes by day and topic
- Supports multiple repositories
- Customizable summary prompt (see `prompt.txt`)
- Copy summary to clipboard with one keypress

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (for building)
- A [Gemini API key](https://aistudio.google.com/app/apikey)

### Build & Install

Clone and build the app:

```sh
git clone https://github.com/yourname/whathaveidone
cd whathaveidone/standup
cargo build --release
```

Or install directly (if published):

```sh
cargo install --git https://github.com/yourname/whathaveidone
```

## Usage

1. Set your Gemini API key:
   ```sh
   export GEMINI_API_KEY=your-key-here
   ```

2. Run the app in your project directory:
   ```sh
   ./target/release/whathaveidone
   # or
   ./target/release/whid
   ```

3. Use the keyboard to navigate:
   - Arrow keys: Move between projects/commits
   - `a`: Show AI summary popup
   - `c`: Copy summary to clipboard
   - `q`: Quit

## Customizing the AI Prompt

Edit the file `prompt.txt` to change how the summary is generated.  
The default prompt includes instructions for grouping by day, topic, and handling ticket numbers.

Example from `prompt.txt`:
```
Overall summary: Summarize all changes in the Git history in short, concise bullet points by grouping similar changes and highlighting their main topics and functions.

- Breakdown by day: Provide a summary of the changes for each day in a single line, highlighting the most important changes and features.
...
```

## Publishing

To let others use your app, push your code to GitHub and share the repository.  
Optionally, publish to [crates.io](https://crates.io/) for easy installation via `cargo install`.
