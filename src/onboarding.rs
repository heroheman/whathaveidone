// src/onboarding.rs
//
// First-run setup wizard. Runs automatically the very first time `whid` starts
// (when no user config exists yet) and on demand via the `--setup` flag, which
// re-runs the wizard and overwrites the fields it manages.
//
// The wizard walks through: welcome → provider → model → API key → language.
// Existing config values are shown greyed-out as defaults; pressing Enter keeps
// them. Providers that already have a saved key are marked in the picker.
// Selecting OpenRouter / Vercel / OpenAI fills in the right base URL and offers
// a curated list of current, sensible models for that provider, with a "type it
// yourself" escape hatch. Built from the same Crossterm primitives as the old
// API-key prompt in `main.rs`, so it adds no new deps.

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

/// Number of numbered steps shown in the header (Provider, Model, Key, Language).
const TOTAL_STEPS: u8 = 4;

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

impl ProviderDef {
    /// Short name for breadcrumbs ("Gemini", "OpenRouter", …).
    fn short(&self) -> &'static str {
        self.label.split_whitespace().next().unwrap_or(self.label)
    }
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

/// One wizard screen: a numbered header plus the breadcrumb of choices so far.
struct Screen<'a> {
    step: u8,
    title: &'a str,
    crumbs: &'a [String],
}

impl Screen<'_> {
    /// Clear the screen and draw the banner, breadcrumb and title.
    fn render(&self) -> anyhow::Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        println!(
            "{}{}",
            " whid setup ".black().on_green().bold(),
            format!("   step {}/{}", self.step, TOTAL_STEPS).dark_grey()
        );
        println!();
        for c in self.crumbs {
            println!("{} {}", "✓".green(), c.as_str().grey());
        }
        if !self.crumbs.is_empty() {
            println!();
        }
        println!("{}", self.title.cyan().bold());
        println!();
        stdout.flush()?;
        Ok(())
    }
}

/// Run the wizard. Prints a "skipped" notice (with how to re-run) on abort.
pub fn run_onboarding(is_reconfigure: bool, settings: &Settings) -> anyhow::Result<bool> {
    let saved = run_wizard(is_reconfigure, settings)?;
    if !saved {
        skip_notice()?;
    }
    Ok(saved)
}

fn run_wizard(is_reconfigure: bool, settings: &Settings) -> anyhow::Result<bool> {
    if !welcome(is_reconfigure)? {
        return Ok(false);
    }

    let cur_provider = settings.provider.as_deref().unwrap_or("gemini");
    let cur_base_url = settings.custom_base_url.as_deref().unwrap_or("");
    let default_provider = current_provider_index(cur_provider, cur_base_url);

    let mut values: Vec<(&'static str, Value)> = Vec::new();
    let mut crumbs: Vec<String> = Vec::new();

    // --- Step 1: Provider ---------------------------------------------------
    let opts: Vec<String> = PROVIDERS.iter().map(|p| p.label.to_string()).collect();
    // Mark providers (except the generic Custom) that already have a saved key.
    let annotations: Vec<String> = PROVIDERS
        .iter()
        .map(|p| {
            if !p.ask_base_url && !stored_key(settings, p.key_field).is_empty() {
                "✓ key saved".green().to_string()
            } else {
                String::new()
            }
        })
        .collect();
    let screen = Screen { step: 1, title: "Choose your AI provider", crumbs: &crumbs };
    let provider_idx = match select(&screen, &opts, default_provider, &annotations)? {
        Some(i) => i,
        None => return Ok(false),
    };
    let provider = &PROVIDERS[provider_idx];
    values.push(("provider", Value::String(provider.config_provider.into())));
    let same_provider = provider_idx == default_provider;

    let cur_model = if provider.config_provider == "gemini" {
        settings.gemini_model.clone()
    } else {
        settings.custom_model.clone().unwrap_or_default()
    };
    let cur_model = if same_provider { cur_model } else { String::new() };

    // Base URL (fixed for gateways, asked only for the generic Custom entry).
    if let Some(url) = provider.base_url {
        if provider.config_provider == "custom" {
            values.push(("custom_base_url", Value::String(url.into())));
        }
    } else if provider.ask_base_url {
        let screen = Screen { step: 1, title: "Custom endpoint base URL", crumbs: &crumbs };
        intro(
            &screen,
            &[
                "Any OpenAI-compatible chat-completions endpoint, e.g.".grey().to_string(),
                "  https://openrouter.ai/api/v1".cyan().to_string(),
                "  https://api.openai.com/v1".cyan().to_string(),
            ],
        )?;
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
    crumbs.push(format!("Provider · {}", provider.short()));

    // --- Step 2: Model ------------------------------------------------------
    let chosen_model = match pick_model(provider, &cur_model, &crumbs)? {
        Some(m) => m,
        None => return Ok(false),
    };
    if provider.config_provider == "gemini" {
        values.push(("gemini_model", Value::String(chosen_model.clone())));
    } else {
        values.push(("custom_model", Value::String(chosen_model.clone())));
    }
    crumbs.push(format!("Model · {chosen_model}"));

    // --- Step 3: API key ----------------------------------------------------
    let cur_key = stored_key(settings, provider.key_field);
    let has_key = !cur_key.is_empty();

    let mut body = Vec::new();
    match provider.key_field {
        "gemini_api_key" => {
            body.push("Get a free Gemini API key:".to_string());
            body.push("  https://aistudio.google.com/apikey".cyan().to_string());
        }
        "openrouter_api_key" => {
            body.push("Create an OpenRouter key:".to_string());
            body.push("  https://openrouter.ai/keys".cyan().to_string());
        }
        "openai_api_key" => {
            body.push("Create an OpenAI key:".to_string());
            body.push("  https://platform.openai.com/api-keys".cyan().to_string());
        }
        "vercel_api_key" => {
            body.push("Create a Vercel AI Gateway key:".to_string());
            body.push("  https://vercel.com/dashboard/ai-gateway".cyan().to_string());
        }
        _ => body.push("Enter the API key for this endpoint.".to_string()),
    }
    body.push(String::new());
    body.push(
        if has_key {
            "Your saved key is shown below — Enter to keep it, or type/paste to replace."
        } else {
            "Input is hidden. You can paste. Leave empty to set it later."
        }
        .grey()
        .to_string(),
    );
    let screen = Screen { step: 3, title: "API key", crumbs: &crumbs };
    intro(&screen, &body)?;

    let new_key = match read_secret("API key", &cur_key)? {
        Some(k) => k,
        None => return Ok(false),
    };
    if provider.config_provider == "gemini" || provider.ask_base_url {
        if !new_key.is_empty() {
            values.push((provider.key_field, Value::String(new_key)));
        }
    } else {
        // Gateway: store under its own field and mirror into the active
        // `custom_api_key` the runtime reads (swapped on every provider switch).
        if !new_key.is_empty() {
            values.push((provider.key_field, Value::String(new_key.clone())));
        }
        values.push(("custom_api_key", Value::String(new_key)));
    }

    // --- Step 4: Language ---------------------------------------------------
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
    let screen = Screen { step: 4, title: "Default language for the summaries", crumbs: &crumbs };
    match select(&screen, &lang_opts, lang_default, &[])? {
        Some(0) => values.push(("lang", Value::String("english".into()))),
        Some(1) => values.push(("lang", Value::String("german".into()))),
        Some(2) => {
            let screen = Screen { step: 4, title: "Language", crumbs: &crumbs };
            intro(&screen, &["Type a language, e.g. \"french\" or \"español\".".grey().to_string()])?;
            let default_lang = if lang_default == 2 { cur_lang.as_str() } else { "" };
            match read_line_default("Language", default_lang)? {
                Some(l) if !l.is_empty() => values.push(("lang", Value::String(l))),
                Some(_) => {}
                None => return Ok(false),
            }
        }
        _ => return Ok(false),
    }

    config::save_config_values(&values)?;
    confirm(&values)?;
    Ok(true)
}

/// Pick a model for `provider`, defaulting to `cur_model` if it matches one of
/// the offered models (otherwise the "Custom" entry, prefilled with cur_model).
fn pick_model(
    provider: &ProviderDef,
    cur_model: &str,
    crumbs: &[String],
) -> anyhow::Result<Option<String>> {
    if provider.models.is_empty() {
        let screen = Screen { step: 2, title: "Model", crumbs };
        intro(&screen, &["Model id for your endpoint, e.g. \"openai/gpt-4o-mini\".".grey().to_string()])?;
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

    let screen = Screen { step: 2, title: "Choose a model", crumbs };
    match select(&screen, &opts, default, &[])? {
        Some(i) if i == custom_idx => {
            let screen = Screen { step: 2, title: "Model", crumbs };
            intro(&screen, &["Enter a model id for this provider.".grey().to_string()])?;
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

    println!("{}", " whid setup ".black().on_green().bold());
    println!();
    if is_reconfigure {
        println!("{}", "Re-running setup. Your current values are preselected.".white());
    } else {
        println!("{}", "Welcome 👋  Let's get you set up.".white());
    }
    println!();
    println!("whid scans the Git repos under your current directory, groups your");
    println!("recent commits, and uses an AI model to write your daily standup.");
    println!();
    println!(
        "{}",
        "Steps: provider → model → API key → language.".grey()
    );
    println!(
        "{}",
        format!("Config: {}", config::get_user_config_path().display()).dark_grey()
    );
    println!();
    println!(
        "Press {} to begin, or {} to skip.",
        "Enter".green().bold(),
        "Esc".red().bold()
    );
    println!(
        "{}",
        "You can re-run this any time with `whid --setup`.".dark_grey()
    );
    stdout.flush()?;

    loop {
        if let Some(k) = read_key()? {
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

/// Arrow-key single-select with a highlighted default ("current") and optional
/// per-option annotations (already styled). Returns the index, or `None`.
fn select(
    screen: &Screen,
    options: &[String],
    default: usize,
    annotations: &[String],
) -> anyhow::Result<Option<usize>> {
    let mut sel = default.min(options.len().saturating_sub(1));
    loop {
        screen.render()?;
        let mut stdout = io::stdout();
        for (i, opt) in options.iter().enumerate() {
            let cur = if i == default { "  (current)".dark_grey().to_string() } else { String::new() };
            let ann = annotations.get(i).filter(|s| !s.is_empty()).map(|s| format!("  {s}")).unwrap_or_default();
            if i == sel {
                println!("  {} {}{}{}", "▶".green(), opt.clone().white().bold(), cur, ann);
            } else {
                println!("    {}{}{}", opt.clone().grey(), cur, ann);
            }
        }
        println!();
        println!("{}", "↑/↓ move · Enter select · Esc skip".dark_grey());
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

/// Draw a screen header then a body block, leaving the cursor ready for input.
fn intro(screen: &Screen, body: &[String]) -> anyhow::Result<()> {
    screen.render()?;
    let mut stdout = io::stdout();
    for line in body {
        println!("{line}");
    }
    println!();
    stdout.flush()?;
    Ok(())
}

/// Final recap of what was saved.
fn confirm(values: &[(&str, Value)]) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    println!("{}", " setup complete ".black().on_green().bold());
    println!();
    let get = |k: &str| values.iter().find(|(key, _)| *key == k).and_then(|(_, v)| v.as_str());
    if let Some(p) = get("provider") {
        println!("  {} {p}", "provider".grey());
    }
    if let Some(m) = get("gemini_model").or_else(|| get("custom_model")) {
        println!("  {} {m}", "model   ".grey());
    }
    if let Some(l) = get("lang") {
        println!("  {} {l}", "language".grey());
    }
    println!();
    println!("Saved to {}", config::get_user_config_path().display().to_string().cyan());
    println!(
        "{}",
        "Re-run any time with `whid --setup`.".dark_grey()
    );
    println!();
    println!("{}", "Starting whid …".white());
    stdout.flush()?;
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}

/// Shown when the wizard is skipped/aborted, so the user knows how to come back.
fn skip_notice() -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
    println!("{}", "Setup skipped.".yellow());
    println!(
        "{}",
        "Run `whid --setup` any time to configure your provider, model and keys."
            .grey()
    );
    println!();
    println!("{}", "Starting whid …".white());
    stdout.flush()?;
    std::thread::sleep(Duration::from_secs(2));
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

/// Core raw-mode line reader with a clearing placeholder and paste support.
/// Returns the trimmed string (or the placeholder if untouched), or `None`.
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
