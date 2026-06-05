// src/onboarding.rs
//
// First-run setup wizard. Runs automatically the very first time `whid` starts
// (when no user config exists yet) and on demand via the `--setup` flag, which
// re-runs the wizard and overwrites the fields it manages.
//
// The wizard walks through: welcome → provider → API key (with the right link
// for the chosen provider) → language, then writes the choices to the user
// config and prints where that file lives. It is built from the same Crossterm
// primitives as the old API-key prompt in `main.rs`, so it adds no new deps.

use std::io::{self, Write};
use std::time::Duration;

use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::Stylize,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use toml::Value;

use crate::config;

/// Result of the wizard: whether anything was written to the user config.
/// `false` means the user aborted (Esc/Ctrl-C) before any choice was saved, so
/// the caller should just continue with whatever defaults are already in place.
pub fn run_onboarding(is_reconfigure: bool) -> anyhow::Result<bool> {
    // --- Welcome ------------------------------------------------------------
    if !welcome(is_reconfigure)? {
        return Ok(false);
    }

    let mut values: Vec<(&'static str, Value)> = Vec::new();

    // --- Provider -----------------------------------------------------------
    let provider = match select(
        "Which AI backend should generate your standup summaries?",
        &[
            "Gemini  (Google AI Studio — recommended)",
            "Custom  (any OpenAI-compatible API: OpenRouter, OpenAI, local …)",
        ],
    )? {
        Some(idx) => idx,
        None => return Ok(false),
    };

    if provider == 0 {
        // ---- Gemini --------------------------------------------------------
        values.push(("provider", Value::String("gemini".into())));
        println_intro(&[
            "Get a free Gemini API key here:".to_string(),
            "  https://aistudio.google.com/apikey".cyan().to_string(),
            String::new(),
            "Paste it below (input is hidden). Leave empty to set it later."
                .grey()
                .to_string(),
        ])?;
        if let Some(key) = read_secret("Gemini API key: ")? {
            if !key.is_empty() {
                values.push(("gemini_api_key", Value::String(key)));
            }
        } else {
            return Ok(false);
        }
    } else {
        // ---- Custom / OpenAI-compatible ------------------------------------
        values.push(("provider", Value::String("custom".into())));
        println_intro(&[
            "Custom provider: any OpenAI-compatible chat-completions endpoint."
                .to_string(),
            "Examples:".grey().to_string(),
            "  https://openrouter.ai/api/v1".cyan().to_string(),
            "  https://api.openai.com/v1".cyan().to_string(),
        ])?;
        match read_line("Base URL [https://openrouter.ai/api/v1]: ")? {
            Some(url) => {
                let url = if url.is_empty() {
                    "https://openrouter.ai/api/v1".to_string()
                } else {
                    url
                };
                values.push(("custom_base_url", Value::String(url)));
            }
            None => return Ok(false),
        }
        match read_line("Model [google/gemini-3.1-flash-lite]: ")? {
            Some(model) => {
                let model = if model.is_empty() {
                    "google/gemini-3.1-flash-lite".to_string()
                } else {
                    model
                };
                values.push(("custom_model", Value::String(model)));
            }
            None => return Ok(false),
        }
        println_intro(&[
            "Paste the API key for this provider (input is hidden).".to_string(),
            "Leave empty to use the CUSTOM_API_KEY environment variable."
                .grey()
                .to_string(),
        ])?;
        if let Some(key) = read_secret("API key: ")? {
            if !key.is_empty() {
                values.push(("custom_api_key", Value::String(key)));
            }
        } else {
            return Ok(false);
        }
    }

    // --- Language -----------------------------------------------------------
    match select(
        "Default language for the summaries?",
        &["English", "German", "Other (type it yourself)"],
    )? {
        Some(0) => values.push(("lang", Value::String("english".into()))),
        Some(1) => values.push(("lang", Value::String("german".into()))),
        Some(2) => match {
            println_intro(&["Type your language, e.g. \"french\" or \"español\".".to_string()])?;
            read_line("Language: ")?
        } {
            Some(lang) if !lang.is_empty() => {
                values.push(("lang", Value::String(lang)))
            }
            Some(_) => {} // empty → keep existing/default
            None => return Ok(false),
        },
        _ => return Ok(false),
    }

    // --- Persist ------------------------------------------------------------
    config::save_config_values(&values)?;

    let path = config::get_user_config_path();
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    println!("{}", "✓ Setup complete.".green().bold());
    println!();
    println!("Your settings were saved to:");
    println!("{}", path.display().to_string().cyan());
    println!();
    println!(
        "{}",
        "Edit that file any time, or run `whid --setup` to redo this wizard."
            .grey()
    );
    println!();
    println!("{}", "Starting whid …".white());
    stdout.flush()?;
    std::thread::sleep(Duration::from_secs(2));

    Ok(true)
}

/// Welcome screen. Returns `false` if the user aborts here.
fn welcome(is_reconfigure: bool) -> anyhow::Result<bool> {
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

    if is_reconfigure {
        println!("{}", "whid — re-run setup".green().bold());
    } else {
        println!("{}", "Welcome to whid 👋".green().bold());
    }
    println!();
    println!("whid scans the Git repos under your current directory, groups your");
    println!("recent commits, and uses an AI model to write your daily standup.");
    println!();
    println!(
        "{}",
        "This quick setup picks a provider, API key and language.".grey()
    );
    println!(
        "{}",
        format!("Config will be stored at {}", config::get_user_config_path().display())
            .grey()
    );
    println!();
    println!(
        "Press {} to begin, or {} to skip setup.",
        "Enter".green(),
        "Esc".red()
    );
    stdout.flush()?;

    loop {
        if let Event::Key(k) = read()? {
            if k.kind == KeyEventKind::Release {
                continue;
            }
            match k.code {
                KeyCode::Enter => return Ok(true),
                KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false)
                }
                _ => {}
            }
        }
    }
}

/// Arrow-key single-select prompt. Returns the chosen index, or `None` on abort.
fn select(title: &str, options: &[&str]) -> anyhow::Result<Option<usize>> {
    let mut stdout = io::stdout();
    let mut sel = 0usize;
    loop {
        execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        println!("{}", title.cyan().bold());
        println!();
        for (i, opt) in options.iter().enumerate() {
            if i == sel {
                println!("  {} {}", "▶".green(), opt.white().bold());
            } else {
                println!("    {}", opt.grey());
            }
        }
        println!();
        println!(
            "{}",
            "↑/↓ to move · Enter to choose · Esc to skip".grey()
        );
        stdout.flush()?;

        if let Event::Key(k) = read()? {
            if k.kind == KeyEventKind::Release {
                continue;
            }
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    sel = (sel + options.len() - 1) % options.len();
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    sel = (sel + 1) % options.len();
                }
                KeyCode::Enter => return Ok(Some(sel)),
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None)
                }
                _ => {}
            }
        }
    }
}

/// Print a short intro block (already on a cleared screen) above an input.
fn println_intro(lines: &[String]) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    for line in lines {
        println!("{line}");
    }
    println!();
    stdout.flush()?;
    Ok(())
}

/// Read a line of visible input in raw mode (so Esc aborts cleanly).
/// Returns `None` on Esc/Ctrl-C, otherwise the trimmed string.
fn read_line(prompt: &str) -> anyhow::Result<Option<String>> {
    read_input(prompt, false)
}

/// Read a line of hidden input, echoing `*` per character.
fn read_secret(prompt: &str) -> anyhow::Result<Option<String>> {
    read_input(prompt, true)
}

fn read_input(prompt: &str, mask: bool) -> anyhow::Result<Option<String>> {
    let mut stdout = io::stdout();
    print!("{}", prompt.cyan());
    stdout.flush()?;

    enable_raw_mode()?;
    let mut buf = String::new();
    let result = loop {
        match read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => match k.code {
                KeyCode::Enter => break Some(buf.trim().to_string()),
                KeyCode::Esc => break None,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    break None
                }
                KeyCode::Backspace => {
                    if buf.pop().is_some() {
                        // Erase the last rendered char.
                        print!("\u{8} \u{8}");
                        stdout.flush()?;
                    }
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    if mask {
                        print!("*");
                    } else {
                        print!("{c}");
                    }
                    stdout.flush()?;
                }
                _ => {}
            },
            _ => {}
        }
    };
    disable_raw_mode()?;
    println!();
    Ok(result)
}
