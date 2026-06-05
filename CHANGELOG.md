# Changelog

All notable changes to this project will be documented in this file.

## [0.6.2] - 2026-06-05

### 🚀 Features

- *(setup)* Add first-run setup wizard with --setup flag
- Add first-run setup wizard with provider/model picker
- *(ui)* Expand help overlay with setup instructions for reconfiguring provider, model, and API keys
- *(stats)* Add commit-type and ticket breakdowns to dashboard
- *(cli)* Add non-interactive direct mode (--list / --generate)
- *(history)* Persist direct generations, cap via recent_generations

### 🐛 Bug Fixes

- *(onboarding)* Enable raw mode during key input to handle arrow/escape keys correctly

### 📚 Documentation

- Reference screenshots from GitHub Pages instead of repo
- Update image links in readme to use absolute URLs
- Document 0.6.2 features (direct mode, setup wizard, stats)

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.1

## [0.6.1] - 2026-06-04

### 📚 Documentation

- Add app screenshots to readme
- Document the Stats view (key 3) in the readme

### ⚙️ Miscellaneous Tasks

- Bump version to 0.6.0

## [0.6.0] - 2026-06-04

### 🚀 Features

- *(ui)* Streamline AI summary workflow
- *(ui)* Compact, responsive layout and context-aware footer
- *(ui)* Colored state chips in the status line
- *(ai)* Integrate custom llm provider
- *(history)* Add persistent overview store
- *(ui)* Persistent AI overview view, replacing the popup
- *(ui)* Top bar + prominent generating banner for overview view
- *(ui)* Bulk-mark a repo's commits from the sidebar
- *(ui)* Persistent top bar + focus ring for view navigation
- *(nav)* Unified two-level keymap, help overlay, inline delete
- *(ui)* Enhance navigation with mouse support and scrolling for commits and overviews
- *(stats)* Add full-screen stats dashboard (key 3)

### 🐛 Bug Fixes

- *(input)* Correct sidebar mouse mapping and remove dead button box
- Honor user filter and highlight hash/date in detailed view
- *(input)* Single correct commit-list click handler
- Reset commit selection on tab switch and stabilize selection order
- Persist marked commits across timeframe changes
- *(utils)* Bounds-check get_active_commits
- *(git)* Harden repository discovery
- *(git)* Apply --to independently and make it day-inclusive
- *(config)* Never discard an invalid user config, report errors clearly
- Tolerate poisoned mutexes instead of crashing the TUI
- Extract commit hash correctly and show subject in detailed view
- *(ui)* Decouple Space detail pane from the detailed-list toggle
- *(ui)* Align overview switch with the tabs, drop count

### 🚜 Refactor

- Drop no-op scrollbar position expression
- Read custom prompt file once per AI summary
- *(input)* Extract single spawn_summary dispatch
- Compute the main layout in one place

### 📚 Documentation

- Add prioritized code-analysis todo list
- Mark code-analysis todo items as resolved
- Record architecture improvements status (V2/V3/V5/V6 done)
- *(todo)* Add Workflow (Phase D) and Layout (Phase E) UX sections
- *(todo)* Mark Phase D and E items done

### ⚡ Performance

- Redraw only on events or while loading

### 🎨 Styling

- Fix clippy warnings
- *(ui)* Softer RGB palette, inner box padding, commit list spacing
- *(ui)* Rounded borders, softer RGB palette, inner box padding

### ⚙️ Miscellaneous Tasks

- Update changelog for version 0.5.1 release and simplify blueprint configuration loading
- Remove unused dependencies and trim tokio features
- *(script)* Update version management script for Rust project

## [0.5.1] - 2025-06-27

### 🐛 Bug Fixes

- *(config)* Simplify blueprint configuration loading

### ⚙️ Miscellaneous Tasks

- Changelog update
- Release whathaveidone version 0.5.0
- Release whathaveidone version 0.5.1

## [0.5.0] - 2025-06-27

### 🚀 Features

- Enhance commit rendering with user filtering and formatting
- *(config)* Implement configuration management for Gemini API key and model
- *(theme)* Introduce theming support for UI components
- *(commits)* Enhance commit retrieval with date filtering options
- *(cli)* Implement command-line interface for commit history summarization
- *(config)* Update configuration options for AI summaries
- *(config)* Improve user configuration handling

### 🐛 Bug Fixes

- Improve date formatting and user filtering in commit retrieval
- *(input)* Improve error message for missing Gemini API key

### 🚜 Refactor

- *(ui)* Simplify item creation in commit rendering
- *(config)* Linting

### 📚 Documentation

- *(readme)* Add configuration section for `whid.toml` and update usage instructions
- *(readme)* Add custom date range feature for commit history

### 🎨 Styling

- *(ui)* Enhance API key prompt with colored output
- *(ui)* Set global background color in commit rendering

### ⚙️ Miscellaneous Tasks

- *(package)* Update gemini-rs to 2.0.0
- *(package)* Add config package
- *(dependencies)* Update and add new packages in Cargo.lock and Cargo.toml
- *(dependencies)* Add toml package to Cargo.toml and Cargo.lock
- *(dependencies)* Add clap package

## [0.4.0] - 2025-05-22

### 🚀 Features

- Add asciicast link to README for better visibility
- Add Stats tab to commit view
- Enhance prompt handling and formatting for commit summaries
- Enhance language support and prompt template placeholders
- Improve git repository detection and UI commit rendering
- Refactor commit view tabs for improved clarity
- Enhance commit view with icons and improved styling
- Enhance selected commits display with icon and improved styling
- Update README for improved clarity and structure
- Add Gemini model selection and update commit summary handling
- Add detailed commit view toggle and enhance commit rendering
- Enhance popup rendering with loading spinner and improved layout
- Add shortcuts visibility toggle and update popup handling
- Update key handling and popup rendering
- Add loading spinner to popup during commit summary fetch
- Add new arguments to handle_key and handle_mouse functions

### 🐛 Bug Fixes

- Update asciicast link format in README for better compatibility
- Update handle_key and handle_mouse functions with new arguments

### ⚙️ Miscellaneous Tasks

- Update after gitignore change
- Update changelog for version 0.4.0
- Release whathaveidone version 0.4.0

## [0.3.2] - 2025-04-25

### 🚀 Features

- Add changelog and configuration for git-cliff
- Update changelog for version 0.3.2

### ⚙️ Miscellaneous Tasks

- Release whathaveidone version 0.3.2

## [0.3.1] - 2025-04-25

### 🚀 Features

- Allow specifying commit history interval with 'today' and 'yesterday'
- Enhance customizable prompt functionality
- Update function signature for fetch_gemini_commit_summary
- Add missing words to cSpell configuration

### ⚙️ Miscellaneous Tasks

- Update .gitignore to include additional files and directories
- Release whathaveidone version 0.3.1

## [0.3.0] - 2025-04-24

### 🚀 Features

- Enhance commit selection and rendering functionality
- Update tab titles and improve tab selection logic
- Refactor commit handling logic for improved tab interaction
- Improve rendering of selected commits with repository grouping
- Update README to remove customization and publishing sections
- Update dependencies in Cargo.toml and Cargo.lock
- Update ratatui dependency version in Cargo.toml
- Remove button box area from commit rendering
- Add once_cell dependency and refactor commit data handling
- Enhance commit selection and rendering logic
- Improve repository list rendering in commits view
- Enhance commit rendering with interval label display
- Display total commit count in repository list
- Update interval labels to English
- Update prompt for commit summary generation
- Validate Gemini API key and enhance error handling
- Add language support for commit summary prompts
- Add language support and interval specification for summaries
- Enhance popup summary with close button functionality
- Implement scrolling functionality for popup summary
- Improve popup text formatting in commit rendering
- Add mouse support for commit list and selection list

### ⚙️ Miscellaneous Tasks

- Release whathaveidone version 0.3.0

## [0.2.0] - 2025-04-21

### 🚀 Features

- Enhance key handling for interval selection
- Update key handling for timeframe navigation
- Enhance commit rendering with syntax highlighting
- Remove unused quote fetching functionality
- Implement commit marking and selection functionality

### ⚙️ Miscellaneous Tasks

- Release whathaveidone version 0.2.0

## [0.1.1] - 2025-04-17

### 🚀 Features

- Initial commit
- Implement commit interval selection in TUI
- Enhance commit display and repository selection in TUI
- Improve repository selection and commit display in TUI
- Enhance commit navigation and detail display in TUI
- Enhance commit display and repository selection in TUI
- Add cSpell configuration for custom words
- Enhance commit rendering and details display
- Implement focus navigation for commit list and details view
- Enhance commit navigation for "All" view and detail display
- Implement scrolling functionality for commit and sidebar views
- Enhance focus navigation for commit selection in sidebar
- Add function to fetch commit details with meta info and file list
- Enhance commit list and sidebar navigation with scrollbars
- Enhance detail view scrolling functionality
- Optimize commit reloading based on interval changes
- Add filtering option for user-specific commits
- Enhance commit filtering by user
- Improve commit navigation and filtering options
- Update dependencies for improved functionality
- Enhance commit navigation and detail view rendering
- Improve commit detail rendering and clear leftover text
- Add Gemini Star Trek quote fetching functionality
- *(TC-3245)* Integrate arboard for clipboard functionality and enhance commit summary
- Adjust popup dimensions and alignment in commit rendering
- Enhance commit summary generation with detailed formatting
- Add initial README for whathaveidone project
- Rename project from standup to whathaveidone
- Simplify imports in input and main modules
- Update installation instructions in README
- Update text and translations in various files
- Update README and Cargo.toml for project clarity
- Update function signatures and improve clarity in network, ui, and utils
- Simplify installation instructions in README

### 🚜 Refactor

- Add models, network fetching, and UI rendering for commit management

### ⚙️ Miscellaneous Tasks

- *(package)* Add "gemini-ai" dependency to Cargo.toml
- Release whathaveidone version 0.1.1

<!-- generated by git-cliff -->
