// src/onboarding.rs
//
// First-run setup wizard. Runs automatically the very first time `whid` starts
// (when no user config exists yet) and on demand via the `--setup` flag, which
// re-runs the wizard and overwrites the fields it manages.
//
// The wizard walks through: welcome → provider → model → API key → language.
// Existing config values are shown greyed-out as defaults; pressing Enter keeps
// them. Selecting OpenRouter / Vercel / OpenAI fills in the right base URL and
// offers a curated list of current, sensible models for that provider, with a
// "type it yourself" escape hatch. It is built from the same Crossterm
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
use crate::config::Settings;

/// A selectable AI backend. Several of these map onto the same config
/// `provider = "custom"` (any OpenAI-compatible gateway) but differ in their
/// base URL and the models they offer.
struct ProviderDef {
    /// Label shown in the provider picker.
    label: &'static str,
    /// Value written to `provider` in the config ("gemini" or "custom").
    config_provider: &'static str,
    /// Fixed base URL for OpenAI-compatible gateways. `None` means either native
    /// (Gemini) or "ask the user" (Custom).
    base_url: Option<&'static str>,
    /// Whether to prompt the user for a base URL (only the generic Custom entry).
    ask_base_url: bool,
    /// Config key under which this provider's API key is stored. Gateways keep
    /// their own key so the wizard can recall it across provider switches.
    key_field: &'static str,
    /// Curated (friendly name, model id) pairs offered for this provider.
    models: &'static [(&'static str, &'static str)],
}

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
const VERCEL_URL: &str = "https://ai-gateway.vercel.sh/v1";
const OPENAI_URL: &str = "https://api.openai.com/v1";

/// Provider catalog, in display order. Model lists are curated to current,
/// cheap-and-fast models that make sense for short standup summaries.
const PROVIDERS: &[ProviderDef] = &[
    ProviderDef {
        label: "Gemini      (Google AI Studio — recommended)",
        config_provider: "gemini",
        base_url: None,
        ask_base_url: false,
        key_field: "gemini_api_key",
        models: &[
            ("Gemini 3.1 Flash Lite", "gemini-3.1-flash-lite"),
            ("Gemini 2.5 Flash", "gemini-2.5-flash"),
            ("Gemini 2.5 Flash Lite", "gemini-2.5-flash-lite"),
            ("Gemini Flash (latest)", "gemini-flash-latest"),
        ],
    },
    ProviderDef {
        label: "OpenRouter  (one key, many models)",
        config_provider: "custom",
        base_url: Some(OPENROUTER_URL),
        ask_base_url: false,
        key_field: "openrouter_api_key",
        models: &[
            ("Gemini 3.1 Flash Lite", "google/gemini-3.1-flash-lite"),
            ("Gemini 2.5 Flash", "google/gemini-2.5-flash"),
            ("Claude Haiku 4.5", "anthropic/claude-haiku-4.5"),
            ("GPT-5 Mini", "openai/gpt-5-mini"),
            ("GPT-4o Mini", "openai/gpt-4o-mini"),
            ("Mistral Small", "mistralai/mistral-small-2603"),
            ("DeepSeek Chat v3.1", "deepseek/deepseek-chat-v3.1"),
        ],
    },
    ProviderDef {
        label: "Vercel      (AI Gateway)",
        config_provider: "custom",
        base_url: Some(VERCEL_URL),
        ask_base_url: false,
        key_field: "vercel_api_key",
        models: &[
            ("Gemini 3.1 Flash Lite", "google/gemini-3.1-flash-lite"),
            ("Gemini 2.5 Flash", "google/gemini-2.5-flash"),
            ("Claude Haiku 4.5", "anthropic/claude-haiku-4.5"),
            ("GPT-5 Mini", "openai/gpt-5-mini"),
            ("GPT-4o Mini", "openai/gpt-4o-mini"),
            ("Mistral Small", "mistral/mistral-small"),
        ],
    },
    ProviderDef {
        label: "OpenAI      (api.openai.com)",
        config_provider: "custom",
        base_url: Some(OPENAI_URL),
        ask_base_url: false,
        key_field: "openai_api_key",
        models: &[
            ("GPT-5 Mini", "gpt-5-mini"),
            ("GPT-5 Nano", "gpt-5-nano"),
            ("GPT-4.1 Mini", "gpt-4.1-mini"),
            ("GPT-4o Mini", "gpt-4o-mini"),
            ("GPT-4o", "gpt-4o"),
        ],
    },
    ProviderDef {
        label: "Custom      (any OpenAI-compatible endpoint)",
        config_provider: "custom",
        base_url: None,
        ask_base_url: true,
        key_field: "custom_api_key",
        models: &[],
    },
];

/// Run the wizard. Existing `settings` supply the greyed-out defaults. Returns
/// whether anything was written (false = aborted before saving).
pub fn run_onboarding(is_reconfigure: bool, settings: &Settings) -> anyhow::Result<bool> {
    if !welcome(is_reconfigure)? {
        return Ok(false);
    }

    // Resolve current selections from settings so each step can default to them.
    let cur_provider = settings.provider.as_deref().unwrap_or("gemini");
    let cur_base_url = settings.custom_base_url.as_deref().unwrap_or("");
    let default_provider = current_provider_index(cur_provider, cur_base_url);

    let mut values: Vec<(&'static str, Value)> = Vec::new();

    // --- Provider -----------------------------------------------------------
    let opts: Vec<String> = PROVIDERS.iter().map(|p| p.label.to_string()).collect();
    let provider_idx = match select(
        "Which AI backend should generate your standup summaries?",
        &opts,
        default_provider,
    )? {
        Some(i) => i,
        None => return Ok(false),
    };
    let provider = &PROVIDERS[provider_idx];
    values.push(("provider", Value::String(provider.config_provider.into())));

    // The model + key currently configured *for this provider kind*, used as
    // defaults only when the user stays on the same provider as before.
    let same_provider = provider_idx == default_provider;
    let cur_model = if provider.config_provider == "gemini" {
        settings.gemini_model.clone()
    } else {
        settings.custom_model.clone().unwrap_or_default()
    };
    let cur_model = if same_provider { cur_model } else { String::new() };

    // --- Base URL -----------------------------------------------------------
    if let Some(url) = provider.base_url {
        // Fixed gateway URL — set silently.
        if provider.config_provider == "custom" {
            values.push(("custom_base_url", Value::String(url.into())));
        }
    } else if provider.ask_base_url {
        println_intro(&[
            "Custom provider: any OpenAI-compatible chat-completions endpoint."
                .to_string(),
            "Examples:".grey().to_string(),
            "  https://openrouter.ai/api/v1".cyan().to_string(),
            "  https://api.openai.com/v1".cyan().to_string(),
        ])?;
        let default_url = if same_provider && !cur_base_url.is_empty() {
            cur_base_url
        } else {
            OPENROUTER_URL
        };
        match read_line_default("Base URL", default_url)? {
            Some(u) => values.push(("custom_base_url", Value::String(u))),
            None => return Ok(false),
        }
    }

    // --- Model --------------------------------------------------------------
    let chosen_model = match pick_model(provider, &cur_model)? {
        Some(m) => m,
        None => return Ok(false),
    };
    if provider.config_provider == "gemini" {
        values.push(("gemini_model", Value::String(chosen_model)));
    } else {
        values.push(("custom_model", Value::String(chosen_model)));
    }

    // --- API key ------------------------------------------------------------
    // Recall the key stored for *this* provider (independent of which provider
    // was active before), so switching back and forth keeps each key.
    let cur_key = stored_key(settings, provider.key_field);
    let has_key = !cur_key.is_empty();

    let mut intro = Vec::new();
    if provider.config_provider == "gemini" {
        intro.push("Get a free Gemini API key here:".to_string());
        intro.push("  https://aistudio.google.com/apikey".cyan().to_string());
    } else if provider.base_url == Some(OPENROUTER_URL) {
        intro.push("Create an OpenRouter key here:".to_string());
        intro.push("  https://openrouter.ai/keys".cyan().to_string());
    } else if provider.base_url == Some(OPENAI_URL) {
        intro.push("Create an OpenAI key here:".to_string());
        intro.push("  https://platform.openai.com/api-keys".cyan().to_string());
    } else if provider.base_url == Some(VERCEL_URL) {
        intro.push("Create a Vercel AI Gateway key here:".to_string());
        intro.push("  https://vercel.com/dashboard/ai-gateway".cyan().to_string());
    } else {
        intro.push("Enter the API key for this provider.".to_string());
    }
    intro.push(String::new());
    if has_key {
        intro.push(
            "Your saved key is shown below — press Enter to keep it, or type to replace."
                .grey()
                .to_string(),
        );
    } else {
        intro.push(
            "Input is hidden. Leave empty to set it later.".grey().to_string(),
        );
    }
    println_intro(&intro)?;

    // The stored key is rendered as a greyed placeholder that clears on the
    // first keystroke; Enter on an untouched field keeps it.
    let new_key = match read_secret("API key", &cur_key)? {
        Some(k) => k,
        None => return Ok(false),
    };
    if provider.config_provider == "gemini" || provider.ask_base_url {
        // Native Gemini or the generic Custom provider: single key field.
        if !new_key.is_empty() {
            values.push((provider.key_field, Value::String(new_key)));
        }
    } else {
        // Gateway: persist under its own field and mirror to the active
        // `custom_api_key` the runtime reads (swapped on every provider switch).
        if !new_key.is_empty() {
            values.push((provider.key_field, Value::String(new_key.clone())));
        }
        values.push(("custom_api_key", Value::String(new_key)));
    }

    // --- Language -----------------------------------------------------------
    let cur_lang = settings.lang.clone().unwrap_or_else(|| "english".into());
    let lang_default = match cur_lang.to_lowercase().as_str() {
        "english" | "en" => 0,
        "german" | "de" | "deutsch" => 1,
        _ => 2,
    };
    let lang_opts = vec![
        "English".to_string(),
        "German".to_string(),
        "Other (type it yourself)".to_string(),
    ];
    match select("Default language for the summaries?", &lang_opts, lang_default)? {
        Some(0) => values.push(("lang", Value::String("english".into()))),
        Some(1) => values.push(("lang", Value::String("german".into()))),
        Some(2) => {
            println_intro(&[
                "Type your language, e.g. \"french\" or \"español\".".to_string(),
            ])?;
            let default_lang = if lang_default == 2 { cur_lang.as_str() } else { "" };
            match read_line_default("Language", default_lang)? {
                Some(l) if !l.is_empty() => values.push(("lang", Value::String(l))),
                Some(_) => {}
                None => return Ok(false),
            }
        }
        _ => return Ok(false),
    }

    // --- Persist ------------------------------------------------------------
    config::save_config_values(&values)?;
    confirm(&values)?;
    Ok(true)
}

/// Pick a model for `provider`, defaulting to `cur_model` if it matches one of
/// the offered models (otherwise the "Custom" entry, prefilled with cur_model).
fn pick_model(
    provider: &ProviderDef,
    cur_model: &str,
) -> anyhow::Result<Option<String>> {
    // Generic Custom provider: no curated list, just ask for the id.
    if provider.models.is_empty() {
        println_intro(&[
            "Model id for your endpoint, e.g. \"openai/gpt-4o-mini\".".to_string(),
        ])?;
        let default = if cur_model.is_empty() { "" } else { cur_model };
        return read_line_default("Model", default);
    }

    let mut opts: Vec<String> = provider
        .models
        .iter()
        .map(|(name, id)| format!("{name}  ·  {id}"))
        .collect();
    opts.push("Custom (type a model id)".to_string());
    let custom_idx = opts.len() - 1;

    let default = provider
        .models
        .iter()
        .position(|(_, id)| *id == cur_model)
        .unwrap_or(custom_idx);

    match select("Which model?", &opts, default)? {
        Some(i) if i == custom_idx => {
            println_intro(&["Enter a model id for this provider.".to_string()])?;
            let prefill = if default == custom_idx { cur_model } else { "" };
            read_line_default("Model", prefill)
        }
        Some(i) => Ok(Some(provider.models[i].1.to_string())),
        None => Ok(None),
    }
}

/// Read the stored key for a given config field out of `settings`.
fn stored_key(settings: &Settings, field: &str) -> String {
    match field {
        "gemini_api_key" => settings.gemini_api_key.clone(),
        "openrouter_api_key" => settings.openrouter_api_key.clone(),
        "vercel_api_key" => settings.vercel_api_key.clone(),
        "openai_api_key" => settings.openai_api_key.clone(),
        "custom_api_key" => settings.custom_api_key.clone(),
        _ => None,
    }
    .unwrap_or_default()
}

/// Map the configured provider + base URL onto an index into `PROVIDERS`.
fn current_provider_index(config_provider: &str, base_url: &str) -> usize {
    if config_provider != "custom" {
        return 0; // Gemini
    }
    if base_url.contains("openrouter") {
        1
    } else if base_url.contains("vercel") {
        2
    } else if base_url.contains("api.openai.com") {
        3
    } else {
        4 // generic Custom
    }
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
        "This quick setup picks a provider, model, API key and language.".grey()
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
        let ev = read_key()?;
        if let Some(k) = ev {
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

/// Arrow-key single-select prompt with a highlighted default (marked "current").
/// Returns the chosen index, or `None` on abort.
fn select(title: &str, options: &[String], default: usize) -> anyhow::Result<Option<usize>> {
    let mut stdout = io::stdout();
    let mut sel = default.min(options.len().saturating_sub(1));
    loop {
        execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        println!("{}", title.cyan().bold());
        println!();
        for (i, opt) in options.iter().enumerate() {
            let marker = if i == default { "  (current)".dark_grey().to_string() } else { String::new() };
            if i == sel {
                println!("  {} {}{}", "▶".green(), opt.clone().white().bold(), marker);
            } else {
                println!("    {}{}", opt.clone().grey(), marker);
            }
        }
        println!();
        println!("{}", "↑/↓ to move · Enter to choose · Esc to skip".grey());
        stdout.flush()?;

        if let Some(k) = read_key()? {
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

/// Final recap of what was saved.
fn confirm(values: &[(&str, Value)]) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    println!("{}", "✓ Setup complete.".green().bold());
    println!();
    let get = |k: &str| values.iter().find(|(key, _)| *key == k).and_then(|(_, v)| v.as_str());
    if let Some(p) = get("provider") {
        println!("  provider : {p}");
    }
    if let Some(m) = get("gemini_model").or_else(|| get("custom_model")) {
        println!("  model    : {m}");
    }
    if let Some(l) = get("lang") {
        println!("  language : {l}");
    }
    println!();
    println!("Saved to {}", config::get_user_config_path().display().to_string().cyan());
    println!(
        "{}",
        "Edit that file any time, or run `whid --setup` to redo this wizard.".grey()
    );
    println!();
    println!("{}", "Starting whid …".white());
    stdout.flush()?;
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}

/// Print a short intro block (clears the screen first) above an input.
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

/// Read one key event in raw mode (rendering stays in cooked mode so `println!`
/// line endings work). Returns `None` for events that aren't a key press.
fn read_key() -> anyhow::Result<Option<crossterm::event::KeyEvent>> {
    enable_raw_mode()?;
    let ev = read();
    disable_raw_mode()?;
    match ev? {
        Event::Key(k) if k.kind != KeyEventKind::Release => Ok(Some(k)),
        _ => Ok(None),
    }
}

/// Read a visible line. The `default` is shown as a greyed placeholder that
/// clears on the first keystroke; Enter on an untouched field keeps it. `None`
/// on Esc/Ctrl-C.
fn read_line_default(prompt: &str, default: &str) -> anyhow::Result<Option<String>> {
    let mut stdout = io::stdout();
    print!("{}", format!("{prompt}: ").cyan());
    stdout.flush()?;
    Ok(read_raw(false, default)?.map(|s| if s.is_empty() { default.to_string() } else { s }))
}

/// Read a hidden line, echoing `*` per character. `placeholder` (the saved key)
/// is shown greyed and clears on the first keystroke; Enter keeps it. `None` on
/// Esc/Ctrl-C.
fn read_secret(prompt: &str, placeholder: &str) -> anyhow::Result<Option<String>> {
    let mut stdout = io::stdout();
    print!("{}", format!("{prompt}: ").cyan());
    stdout.flush()?;
    read_raw(true, placeholder)
}

/// Core raw-mode line reader with a clearing placeholder. Returns the trimmed
/// string (or the placeholder if the field was left untouched), or `None` on
/// abort.
fn read_raw(mask: bool, placeholder: &str) -> anyhow::Result<Option<String>> {
    use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};

    let mut stdout = io::stdout();
    enable_raw_mode()?;
    // Pastes arrive as a single `Event::Paste` instead of a flood of key events,
    // so long keys can be pasted in one go.
    execute!(stdout, EnableBracketedPaste)?;

    // Render the placeholder to the right of the cursor, then move back so the
    // first typed character overwrites it.
    let mut placeholder_shown = false;
    if !placeholder.is_empty() {
        print!("{}", placeholder.dark_grey());
        let width = placeholder.chars().count() as u16;
        execute!(stdout, cursor::MoveLeft(width))?;
        placeholder_shown = true;
    }

    let clear_placeholder = |shown: &mut bool, out: &mut io::Stdout| -> anyhow::Result<()> {
        if *shown {
            execute!(out, Clear(ClearType::UntilNewLine))?;
            *shown = false;
        }
        Ok(())
    };

    let mut buf = String::new();
    let result = loop {
        match read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => match k.code {
                KeyCode::Enter => {
                    // Untouched field with a placeholder → keep it.
                    if placeholder_shown && buf.is_empty() {
                        break Some(placeholder.to_string());
                    }
                    break Some(buf.trim().to_string());
                }
                KeyCode::Esc => break None,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    break None
                }
                KeyCode::Backspace => {
                    clear_placeholder(&mut placeholder_shown, &mut stdout)?;
                    if buf.pop().is_some() {
                        print!("\u{8} \u{8}");
                        stdout.flush()?;
                    }
                }
                KeyCode::Char(c) => {
                    clear_placeholder(&mut placeholder_shown, &mut stdout)?;
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
            // Bracketed paste: insert the whole clipboard at once, stripping any
            // control chars/newlines so a multi-line paste stays one line.
            Event::Paste(s) => {
                clear_placeholder(&mut placeholder_shown, &mut stdout)?;
                let clean: String = s.chars().filter(|c| !c.is_control()).collect();
                buf.push_str(&clean);
                if mask {
                    print!("{}", "*".repeat(clean.chars().count()));
                } else {
                    print!("{clean}");
                }
                stdout.flush()?;
            }
            _ => {}
        }
    };
    execute!(stdout, DisableBracketedPaste)?;
    disable_raw_mode()?;
    println!();
    Ok(result)
}
