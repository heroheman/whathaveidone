# TODO — Code-Analyse whathaveidone

Sortiert nach Priorität. Verbesserungsvorschläge (Architektur) sind ans Ende verschoben und werden vorerst **nicht** umgesetzt.

## Phase A — Cleanup / Quick Wins (geringes Risiko)

- [ ] A1 — Ungenutzte Dependencies entfernen: `reqwest`, `serde_json`, `futures`, `tokio-util`, `tui-scrollview`, `shellexpand`. `tokio` features `full` → minimal. Platzhalter-`authors` in Cargo.toml korrigieren.
- [ ] A2 — clippy-Warnungen beheben: `.get(0)`→`.first()` (network.rs), überflüssige `as u16`-Casts, `splitn` ohne Grund, `else { if }`-Kollaps, `unused_enumerate_index`, redundanter Import, `unneeded return`, `map_or`, elidierbare Lifetime, `&Vec`→`&[_]`.
- [ ] A3 — No-Op-Scrollbar-Ausdruck `saturating_sub(visible.saturating_sub(visible))` entfernen (ui.rs:330, 353).
- [ ] A4 — Toten Code in utils.rs (5× `#[allow(dead_code)]`) entfernen oder dokumentieren.
- [ ] A5 — Prompt-Datei wird im `a`-Handler doppelt gelesen → einmal lesen.

## Phase B — Korrektheits-Bugs

- [ ] B1 — `s`-Taste ist No-Op, Bookmark-Popup per Tastatur unerreichbar (input.rs:96). `s` soll das Selektions-Popup öffnen.
- [ ] B2 — Maus-Handler referenziert entfernte Sidebar-Button-Box → Klicks auf untere Sidebar-Zeilen lösen ungewollt AI-Summary/Bookmark aus (input.rs:497-557).
- [ ] B3 — Maus-AI-Summary baut kaputten Prompt (keine Platzhalter-Substitution, kein Fallback auf prompt_en) (input.rs:508-540).
- [ ] B4 — Detail-Modus ignoriert `filter_by_user` (git.rs:69-72); Header zeigt trotzdem "only mine".
- [ ] B5 — `render_commit_line` zerbricht im Detail-Modus (splittet auf `|`, Detail-Format nutzt keine `|`) (ui.rs:26).
- [ ] B6 — Maus-Klick-Mapping nutzt manuelles `commitlist_scroll`, Rendering ignoriert es → Klicks treffen falschen Commit bei gescrollter Liste (input.rs:559, 660).
- [ ] B7 — Sidebar-Maus rechnet 1 Zeile/Repo, gerendert werden 3 Zeilen/Repo → falsches Repo selektiert (input.rs:559 vs ui.rs:234).
- [ ] B8 — Selection-Tab: Index-Mismatch (Header-Zeilen dazwischen), stale Index bei Tab-Wechsel, nicht-deterministische HashSet-Reihenfolge (ui.rs:388, input.rs:329).
- [ ] B9 — Selektion an aktuelles Timeframe gekoppelt: markierte Commits fallen still raus (input.rs:329-331).

## Phase C — Robustheit

- [ ] C1 — Möglicher Panic in `get_active_commits` (Out-of-bounds Index) + unsinnige Logik (utils.rs:9-18).
- [ ] C2 — Mutex-Poisoning crasht App (`lock().unwrap()` überall).
- [ ] C3 — Stiller Datenverlust bei beschädigter User-Config (config.rs:35-36 überschreibt).
- [ ] C4 — `Settings::new()`/`config.rs` panicken mit nichtssagenden Meldungen.
- [ ] C5 — `find_git_repos` rekursiert unbegrenzt tief, `to_str().unwrap()`-Panic, `entry?` bricht Scan ab (git.rs:14-25).
- [ ] C6 — `--to` ohne `--from` wird ignoriert; YYYY-MM-DD-Datumssemantik bei `--until` (git.rs:57-67).

---

## Verbesserungsvorschläge (Architektur) — VORERST AUSGESETZT

- V1 — Zentrales `App`-Struct statt ~24 `&mut`-Parameter durch main.rs → input.rs (Wurzel von B2/B6/B7).
- V2 — Layout einmal berechnen und an Render + Input geben (statt 4× duplizieren).
- V3 — Beide Async-Pfade (Maus/Tastatur AI-Summary) auf eine gemeinsame Funktion vereinheitlichen.
- V4 — `CommitData` strukturieren: `struct Commit { hash, datetime, author, subject, body }` statt roher `Vec<String>` + verstreutes `split('|')`.
- V5 — Konstante Redraw-Schleife (~33fps im Leerlauf) event-/loading-gesteuert machen.
- V6 — Fehlerbehandlung statt Panics (Poison-Recovery, anyhow-Kontext).
