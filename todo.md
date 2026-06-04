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

## Phase D — Workflow (UI/UX)

Der Kern-Flow *Commits auswählen → Zusammenfassung erzeugen → kopieren* hat zu viele Schritte und zwei parallele Wege.

- [x] D1 — **Kopier-Feedback.** Popup-Fußzeile bestätigt „✓ Copied to clipboard" nach `c`.
- [x] D2 — **Redundante Selektion entfernt.** `s`-Popup gestrichen; `s` springt jetzt in den Selection-Tab.
- [x] D3 — **Auto-Copy / Enter.** `Enter` im Popup kopiert + schließt in einem Schritt.
- [x] D4 — **Loading-Popup entrümpelt.** Debug-Variablen hinter `--debug`; sonst „Summarizing N commits from {project}…".
- [x] D5 — **Abbrechen/Regenerieren.** `Esc` canceln (löst Spinner-Loop + droppt das fetch-Future), `r` regeneriert die letzte Summary.

## Phase E — Layout (UI/UX)

- [x] E1 — **Sidebar 1 Zeile/Repo** (Name + Count). `ListState`-Index korrigiert (`i+2` statt fehlerhaftem `*3+2`), Auto-Scroll funktioniert; Maus-Hit-Test angepasst.
- [x] E2 — **Responsive Sidebar-Breite** (~¼ der Terminalbreite, geklemmt auf 22–36), Divider skaliert mit.
- [x] E3 — **Kontextabhängige Footer-Hints** je nach Fokus (Sidebar/Liste/Detail) bzw. offenem Popup.
- [ ] E4 — **Stats-Tab** echt befüllen oder ganz entfernen. **VERTAGT** (später).
- [x] E5 — **Detail-Pane** nutzt jetzt `block.inner()` statt manuellem Space-Blanking.

---

## Verbesserungsvorschläge (Architektur) — VORERST AUSGESETZT

Erledigt: V5, V3, V2. V6 wurde durch Phase C abgedeckt. Offen (bewusst vertagt): V1, V4.

- [ ] V1 — Zentrales `App`-Struct statt ~24 `&mut`-Parameter durch main.rs → input.rs (Wurzel von B2/B6/B7). **OFFEN.** Großer mechanischer Refactor; löst die verbleibenden `clippy::too_many_arguments` (handle_key 24/7, render_commits 19/7, handle_mouse 8/7). Empfehlung: auf Branch + manuelle TUI-Tests.
- [x] V2 — Layout einmal berechnen (`ui::compute_layout`), von render_commits + beiden main.rs-Hit-Tests genutzt.
- [x] V3 — Async vereinheitlicht: Maus-Pfad entfernt, eine `spawn_summary`-Dispatch-Stelle.
- [ ] V4 — `CommitData` strukturieren: `struct Commit { hash, datetime, author, subject, body, raw }` statt roher `Vec<String>` + verstreutes `split('|')`. **OFFEN.** Verhaltenserhaltend via `raw` möglich, aber breit streuend (git/ui/input/models). Empfehlung: auf Branch + manuelle TUI-Tests.
- [x] V5 — Redraw event-/loading-gesteuert statt konstant ~33fps.
- [x] V6 — Gefährliche Panics/`unwrap` an Rändern beseitigt (Phase C); verbleibende zwei `expect` (eingebettetes Blueprint-Asset, `home_dir`) sind vertretbar.
