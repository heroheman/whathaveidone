# TODO — Code-Analyse whathaveidone

Sortiert nach Priorität. Verbesserungsvorschläge (Architektur) sind ans Ende verschoben und werden vorerst **nicht** umgesetzt.

## Phase A — Cleanup / Quick Wins (geringes Risiko)

- [x] A1 — Ungenutzte Dependencies entfernt (`reqwest`, `serde_json`, `futures`, `tokio-util`, `tui-scrollview`, `shellexpand`), `tokio` features minimiert, `authors` korrigiert.
- [x] A2 — clippy-Warnungen behoben (`.first()`, redundante Casts/Imports/return, `splitn`, `else { if }`, `unused_enumerate_index`, Slice-Parameter etc.).
- [x] A3 — No-Op-Scrollbar-Ausdruck `saturating_sub(visible.saturating_sub(visible))` entfernt.
- [x] A4 — Tote utils.rs-Helfer: laut CLAUDE.md bewusst als Referenz behalten → **keine Änderung**.
- [x] A5 — Prompt-Datei wird im `a`-Handler nur noch einmal gelesen.

## Phase B — Korrektheits-Bugs

- [x] B1 — `s`-Taste öffnet/schließt jetzt das Bookmark-Popup.
- [x] B2 — Phantom-Button-Box im Maus-Handler entfernt.
- [x] B3 — Kaputter Maus-AI-Summary-Pfad mit B2 entfernt (Tastatur-`a` bleibt korrekt).
- [x] B4 — Detail-Modus respektiert `filter_by_user`.
- [x] B5 — `render_commit_line` hebt Hash/Datum auch im Detail-Modus hervor.
- [x] B6 — Ein korrekter Commit-Listen-Klick-Handler (Tab-/Border-Offset, Header-Zeilen), `commitlist_scroll`-Müll entfernt.
- [x] B7 — Sidebar-Maus rechnet mit 3 Zeilen/Repo, bound auf gerenderte Repos.
- [x] B8 — Commit-Index wird bei echtem Tab-Wechsel zurückgesetzt; Bookmark-Popup & AI-Prompt deterministisch geordnet.
- [x] B9 — Markierte Commits überleben Timeframe-Wechsel (Repo + volle Zeile gespeichert).

## Phase C — Robustheit

- [x] C1 — `get_active_commits` bounds-checked (`.get()` statt Index-Panic).
- [x] C2 — Mutex-Poisoning toleriert (`LockExt::lock_safe`), kein App-Crash mehr.
- [x] C3 — Beschädigte User-Config wird nicht mehr stillschweigend überschrieben, klare Fehlermeldung.
- [x] C4 — `Settings`/Config-Laden ohne `expect()`-Panics, aussagekräftige Fehler.
- [x] C5 — `find_git_repos`: `&Path` (kein UTF8-Panic), überspringt unlesbare Einträge/Symlinks/Hidden-Dirs, sortiert deterministisch.
- [x] C6 — `--to` wirkt unabhängig von `--from`, bare YYYY-MM-DD ist tagesinklusiv (23:59:59).

---

## Verbesserungsvorschläge (Architektur) — VORERST AUSGESETZT

Erledigt: V5, V3, V2. V6 wurde durch Phase C abgedeckt. Offen (bewusst vertagt): V1, V4.

- [ ] V1 — Zentrales `App`-Struct statt ~24 `&mut`-Parameter durch main.rs → input.rs (Wurzel von B2/B6/B7). **OFFEN.** Großer mechanischer Refactor; löst die verbleibenden `clippy::too_many_arguments` (handle_key 24/7, render_commits 19/7, handle_mouse 8/7). Empfehlung: auf Branch + manuelle TUI-Tests.
- [x] V2 — Layout einmal berechnen (`ui::compute_layout`), von render_commits + beiden main.rs-Hit-Tests genutzt.
- [x] V3 — Async vereinheitlicht: Maus-Pfad entfernt, eine `spawn_summary`-Dispatch-Stelle.
- [ ] V4 — `CommitData` strukturieren: `struct Commit { hash, datetime, author, subject, body, raw }` statt roher `Vec<String>` + verstreutes `split('|')`. **OFFEN.** Verhaltenserhaltend via `raw` möglich, aber breit streuend (git/ui/input/models). Empfehlung: auf Branch + manuelle TUI-Tests.
- [x] V5 — Redraw event-/loading-gesteuert statt konstant ~33fps.
- [x] V6 — Gefährliche Panics/`unwrap` an Rändern beseitigt (Phase C); verbleibende zwei `expect` (eingebettetes Blueprint-Asset, `home_dir`) sind vertretbar.
