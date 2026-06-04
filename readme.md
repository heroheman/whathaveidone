# whathaveidone

A terminal tool to summarize your Git commit history for daily standups, using AI. Works with Google Gemini out of the box, or with any OpenAI-compatible API (OpenRouter, the Vercel AI Gateway, a local server, OpenAI itself, …).

<a href="https://asciinema.org/a/l58gl6wettdA3x4eLD4jCkWkq" target="_blank"><img src="https://asciinema.org/a/l58gl6wettdA3x4eLD4jCkWkq.svg" /></a>

---

## Features
- Summarizes Git commit history for one or more projects
- Groups changes by day and topic
- Supports multiple repositories
- Two views with a consistent keymap: a **Commits** view (`1`) and a persistent **Overviews** view (`2`)
- AI overviews are saved to disk and survive restarts — browse, copy, regenerate, or delete past summaries in a master/detail layout
- Customizable summary prompt
- Copy summary to clipboard with one keypress
- Mark individual commits with `m`, or bulk-mark a whole repo's commits from the sidebar, then summarize just the selection
- Compact, responsive layout with a top bar, a focus ring, and a context-aware footer
- Built-in `?` help overlay listing every key

---

## Installation

### Prerequisites
- [Rust](https://rustup.rs/) (for building)
- An API key for your chosen provider — a [Gemini API key](https://aistudio.google.com/app/apikey), or a key for any OpenAI-compatible service (e.g. [OpenRouter](https://openrouter.ai/), the [Vercel AI Gateway](https://vercel.com/docs/ai-gateway), or OpenAI).

### Build & Install
```sh
cargo install whathaveidone
```

### Set up your API key

**Gemini (default):**
```sh
export GEMINI_API_KEY=your-key-here
```
On first run, if no key is found, the app also offers to save one to `~/.config/whid/whid.toml`.

**Custom OpenAI-compatible providers:**
```sh
export CUSTOM_API_KEY=your-key-here
```
You can also store the key directly in your config as `custom_api_key`.

Add the relevant `export` line to your shell profile (e.g. `~/.zshrc`) to make it persistent across terminal sessions.

---

## Configuration

`whathaveidone` uses a TOML file for configuration, allowing you to customize its behavior.

### Configuration Hierarchy

The application loads settings from up to three locations in the following order of precedence (lower numbers are overridden by higher numbers):

1.  **Project Blueprint (`whid.toml`)**: A `whid.toml` file located in the project's root directory serves as the base configuration. This file is required and acts as a blueprint for user-specific settings.
2.  **User Configuration (`~/.config/whid/whid.toml`)**: On the first run, the application copies the project's `whid.toml` to a user-specific directory. This file stores your personal default settings.
3.  **Local Override (`whid.toml`)**: You can place a `whid.toml` file in the directory where you run the `whid` command. Its settings will override any of the above, which is useful for project-specific configurations.

### Available Settings

Here's an example of the `whid.toml` file and the available settings:

```toml
# Which AI backend to use: "gemini" (default) or "custom".
# Can be overridden by the --provider command-line flag.
provider = "gemini"

# The default Gemini model to use for summaries (when provider = "gemini").
# This can be overridden by the --model command-line flag.
gemini_model = "gemini-3.1-flash-lite"

# --- Custom OpenAI-compatible provider settings (used when provider = "custom") ---
# Base URL of the endpoint, e.g. https://openrouter.ai/api/v1 or https://api.openai.com/v1.
# Can be overridden by the --base-url command-line flag.
custom_base_url = ""

# Model name for the custom provider, e.g. "openai/gpt-4o-mini".
# Can be overridden by the --model command-line flag.
custom_model = ""

# API key for the custom provider.
# If empty, the CUSTOM_API_KEY environment variable is used instead.
custom_api_key = ""

# Optional: Path to a custom prompt template file.
# If provided, this file will be used for AI summaries.
# This can be overridden by the --prompt command-line flag.
custom_prompt_path = "path/to/your/prompt.txt"

# Default language for the AI summary.
# Can be overridden by the --lang command-line flag.
lang = "english"
```

---

## Usage

Run the app in your terminal:
```sh
whathaveidone
# or
whid
```

### Views & navigation

The app has two top-level views and one consistent key model that separates *which view* you're in from *which pane* has focus:

- **Commits** (`1`) — the sidebar of repositories plus the commit list. Switch the list between **Timeframe** and **Selection** mode with `s`.
- **Overviews** (`2`) — a master/detail browser of past AI summaries (dimmed until you generate one). The left list holds every saved overview with its creation metadata; the right pane shows the full text.

Everywhere: `Tab` / `Shift+Tab` move focus between panes, arrows or `h j k l` navigate and scroll within the focused pane, `?` toggles a help overlay listing every key, and `q` quits.

Press `a` to generate an AI overview. The app switches to the Overviews view with a spinner; when it finishes, the summary becomes the newest saved entry (persisted to disk) so it's still there after a restart.

### Provider & model selection

By default `whathaveidone` uses Google **Gemini**. You can switch to any **custom OpenAI-compatible** API (OpenRouter, Vercel AI Gateway, a local server, OpenAI itself) by setting `provider = "custom"` in your `whid.toml`, or with the `--provider` flag.

**Gemini model selection**

Set `gemini_model` in your config, or use `--model <model>` as a command-line override. The default is `gemini-3.1-flash-lite`.

```sh
whathaveidone --model gemini-1.5-pro
# or
whathaveidone --model gemini-2.5-flash-preview-05-20
```

**Custom OpenAI-compatible provider**

Set `provider = "custom"`, a `custom_base_url`, and a `custom_model` in your config — or pass them on the command line. The API key comes from `custom_api_key` in the config or the `CUSTOM_API_KEY` environment variable.

```sh
# OpenRouter example
export CUSTOM_API_KEY=sk-or-...
whathaveidone --provider custom \
  --base-url https://openrouter.ai/api/v1 \
  --model openai/gpt-4o-mini

# OpenAI example
export CUSTOM_API_KEY=sk-...
whathaveidone --provider custom \
  --base-url https://api.openai.com/v1 \
  --model gpt-4o-mini
```

The selected provider and model are shown in the Overviews view (and in the `--debug` output) while waiting for the AI response.

### Language selection
To use a specific language for the AI summary, add the `--lang <language>` parameter:
```sh
whathaveidone --lang german      # German
whid --lang english              # English (default)
```
**Note:** The translation is performed by Gemini itself. The `--lang` command-line flag will always override the `lang` setting from your configuration file.

### Time interval selection
Use `[` or `]` to cycle the time interval (previous / next). `Tab` / `Shift+Tab` move focus between panes instead.

Alternatively, you can specify the start interval for commit history as a parameter:
- `24` or `today` (default)
- `48`
- `72` or `yesterday`
- `week`
- `month`

Example for 1 week in German:
```sh
whathaveidone week --lang german
```

#### Custom Date Range
You can specify a custom date range for the commit history using the `--from` and `--to` parameters. The date format is `YYYY-MM-DD`.

- `--from YYYY-MM-DD`: Start date for the commit history.
- `--to YYYY-MM-DD`: End date for the commit history. If not provided, it defaults to the current date.

Example:
```sh
whathaveidone --from 2023-01-01 --to 2023-01-31
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

The `--prompt` command-line flag will always override the `custom_prompt_path` from your configuration file.

### Detailed commit view ("git log" style)

You can toggle a detailed, multi-line commit log view (similar to `git log --format` output) by pressing the `d` key in the commit list. This view shows the full commit message body and author for each commit, formatted in a pretty-printed, multi-line style.

**Note:** The detailed view is only recommended for smaller datasets (shorter timeframes or a single project). For large repositories or long timeframes, the output may be too large for the AI model to summarize effectively.

---

## Keyboard Shortcuts

Press `?` at any time for an in-app overlay of all keys.

### Global
- `1` / `2`: Switch view (Commits / Overviews)
- `Tab` / `Shift+Tab`: Move focus between panes
- `←` `→` / `h` `l`: Move focus left / right
- `↑` `↓` / `j` `k`: Navigate / scroll in the focused pane
- `?`: Toggle the help overlay
- `q`: Quit

### Commits view
- `Space`: Open/close the detail pane
- `s`: Toggle list mode (Timeframe / Selection)
- `m`: Mark/unmark a commit — or, with the sidebar focused, mark/unmark the whole repo's commits
- `[` / `]`: Previous / next timeframe
- `u`: Toggle mine / all authors
- `d`: Toggle detailed commit view (multi-line, git log style)
- `a`: Generate an AI overview

### Overviews view
- `Enter` / `c`: Copy the selected overview to the clipboard
- `r`: Regenerate the last overview
- `x` / `Del`: Delete the selected overview (asks `y`/`n` to confirm)
- `a`: Generate a new overview
- `Esc`: Back to Commits (cancels an in-flight generation)

---

## Links
- [whathaveidone on Crates.io](https://crates.io/crates/whathaveidone)
- [whathaveidone on GitHub](https://github.com/heroheman/whathaveidone)

## Development
_coming soon_