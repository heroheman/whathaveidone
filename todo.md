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

Offene `clippy::too_many_arguments`-Warnungen (handle_key 24/7, render_commits 19/7, handle_mouse 8/7) lösen sich mit V1 auf.

- V1 — Zentrales `App`-Struct statt ~24 `&mut`-Parameter durch main.rs → input.rs (Wurzel von B2/B6/B7).
- V2 — Layout einmal berechnen und an Render + Input geben (statt mehrfach duplizieren); ermöglicht persistenten Scroll-Offset → exaktes Klick-Mapping auf gescrollten Listen.
- V3 — Beide Async-Pfade (Maus/Tastatur AI-Summary) auf eine gemeinsame Funktion vereinheitlichen.
- V4 — `CommitData` strukturieren: `struct Commit { hash, datetime, author, subject, body }` statt roher `Vec<String>` + verstreutes `split('|')`.
- V5 — Konstante Redraw-Schleife (~33fps im Leerlauf) event-/loading-gesteuert machen.
- V6 — Weitere Fehlerbehandlung statt verbleibender Panics/`unwrap` an Rändern.
