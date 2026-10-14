# Notepad++ conversion: remaining feature checklist

Audited against the **bundled Notepad++ 8.9.8.1 source** on **2026-10-14** and the current working tree, including the new Scintilla/Lexilla integration. This is a source-based gap audit, not a claim that every existing command has been exercised interactively. It does not compare against a different online release.

Unchecked entries mean missing functionality or incomplete upstream behavior. **Partial** means a working subset already exists; implement the stated remainder rather than replacing that subset. **Verify** identifies a compatibility risk requiring confirmation, not a proven missing feature. The command appendix lists individual missing upstream menu routes, including partial alternatives. Preference entries enumerate the remaining settings by upstream page.

Plugins are recorded separately for completeness, but remain **outside the previously requested implementation scope**. macOS/Linux work is a separate conversion requirement: upstream Notepad++ itself is Windows-only.

## Audit sources and exclusions

- Upstream: `notepad_plus_plus/PowerEditor/src/Notepad_plus.rc`, `menuCmdID.h`, `NppCommands.cpp`, `Notepad_plus.cpp`, `Parameters.cpp`, `winmain.cpp`, `ScintillaComponent/`, `WinControls/Preference/preference.rc`, other `WinControls/` dialogs, and bundled Scintilla/Lexilla.
- Conversion: production code in `Windows/src/`, `crates/editor-core/src/`, and `crates/npp-compat/src/`; platform directories; existing README and parity notes were cross-checked because they contain stale statements.
- The bundled resource contains 479 distinct non-plugin main-menu command IDs. Rechecking the current production dispatcher and language registry finds 156 without literal references; the older `docs/menu-inventory.csv` reports 192. Neither number is a missing-feature count: ranges, aliases, and partial replacements need interpretation. Comments, constants, tests and compiled upstream code do not establish application integration.
- Already implemented, so not listed as wholly missing: basic file editing/saving/reloading; tabs and tab ordering; manual session XML; Unicode plus explicit Windows-1251/1252 and Shift-JIS; normal/extended/regex search subsets; Find in Files; bookmark operations; line/case/whitespace operations; hashes; basic printing; basic macros/Run; the 94 built-in language choices, highlighting, native folding, auto-indent and formatting.
- Numbered tabs 1–9 and fold/unfold levels 1–8 have range handlers. Tab-to-start/end has rewrite aliases. Do not re-add these as absent solely because their individual upstream IDs are not present in the dispatcher.
- Built-in Scintilla gestures may already work without Rust handlers. The missing integration of multiple selections, markers and history is distinguished below from native engine capability.

## 1. File operations, sessions and recovery

- [x] Open several selected files from one Open dialog; the current dialog returns one path. Retain ordering and report per-file failures.
- [x] Open a command prompt in the current file's directory (`IDM_FILE_OPEN_CMD`).
- [x] Open PowerShell in the current file's directory (`IDM_FILE_OPEN_POWERSHELL`).
- [ ] Open the containing folder as a workspace; open an arbitrary folder as a workspace.
- [ ] Folder drag-and-drop behavior: workspace opening versus recursive file opening, controlled by preferences.
- [ ] Pin/unpin documents, pin-aware ordering and close behavior, and Close All but Pinned Documents.
- [x] Persist closed-file history across launches, including explicit encoding and lossless Windows paths, for Restore Last Closed File.
- [x] Recent Files submenu with individual reopening, Open All in oldest-to-newest order and persistent Clear History; failures remain retryable.
- [x] Configurable recent-file limit (0–30); zero disables collection, smaller limits trim immediately, and startup restores with the configured limit.
- [x] Recent history labels: full path, filename only or custom compact-path length (1–259), persisted and applied immediately.
- [ ] **Partial:** full upstream recent-file reopen semantics and interactive failure/retry verification. Startup existence checking and direct-in-File/submenu placement are implemented; verify responsiveness for inaccessible/network paths.
- [ ] **Partial:** automatic external-change/deletion notifications and configurable silent reload. Existing activation/pre-save checks do not supply the complete watching workflow.
- [ ] Tail/monitor mode, append updates, auto-scroll, truncation/rotation handling and monitoring-specific read-only behavior.
- [ ] Save/relaunch with administrator privileges for permission-denied files; preserve the document and its state through elevation.
- [ ] **Partial:** configurable normal/simple/verbose backups and custom backup directory. Atomic-write recovery files are not the upstream user-facing backup policy.
- [ ] Periodic snapshots of unsaved named and untitled documents, configurable interval and backup location.
- [ ] Restore the previous session automatically on launch and save it on shutdown.
- [ ] Restore unsaved content from snapshots after a crash/restart; provide emergency recovery behavior.
- [ ] Retain missing/inaccessible session files and restore them when available, rather than simply omitting or reporting them.
- [x] Capture/restore collapsed folds in manual session XML, using upstream Fold line entries. In-memory fold retention across tab switches also works.
- [x] Retain horizontal scroll across tab switches and persist/restore xOffset and scrollWidth in manual sessions.
- [ ] **Partial:** persist independent view positions, tab colors/pins, document-map state and workspace/panel state with sessions.
- [ ] **Partial:** restore two session views independently; both upstream view lists currently share one tab strip.
- [ ] Session-file extension recognition and session-opening preferences, including opening a session in a new instance.
- [ ] **Partial:** match upstream permissive recovery of malformed/duplicate session nodes where appropriate; the current XML reader rejects some inputs upstream tolerates.
- [ ] Persist and restore window size/position, maximized state, view arrangement and docking layout.

## 2. Editor behavior and selection

- [ ] Integrate rectangular selection with the Rust document model, copy/cut/paste, replacement, undo/redo, macros and status information.
- [x] Rectangular clipboard paste recognizes Microsoft/Borland markers, uses length-aware native column insertion with short-line padding, retains embedded NUL and records one model text transaction/macro change. Interactive verification and selection-aware undo remain pending.
- [x] Copy/Cut all native selection ranges, including rectangular selection text/virtual padding and the Windows `MSDEVColumnSelect` clipboard marker. Cut preserves intervening source text/EOLs in one model transaction and records its text change for macros. Selection-aware undo and clipboard interoperability verification remain pending.
- [x] Preserve native rectangular/multiple selection modes, endpoints, direction and virtual space across ordinary tab switches using Scintilla selection serialization. Model-driven navigation takes precedence; interactive verification remains pending.
- [x] Begin/End Select in Column Mode with edit-tracked shared view anchor, native rectangle, checked pending command and mutual exclusion with ordinary Begin/End Select. Rectangular editing/model integration remains pending.
- [x] Column Mode help dialog explains upstream selection gestures and explicitly identifies incomplete rectangular-editing integration.
- [ ] Column Editor: insert repeated text, generate numbers, initial/increment/repeat values, numeric formats and leading-zero behavior.
- [ ] Integrate multiple carets/selections with the Rust model; prevent refresh/undo from reducing them to one ordered range.
- [ ] **Verify clipboard shape parity:** rerun the `alpha beta alpha` multi-select/copy/tab-switch/paste scenario against upstream. Earlier expected stream-paste output was incorrect for upstream multi-range clipboard data, which is column-shaped. Keyboard routing and shape metadata are now corrected in source; compare actual column insertion and NUL/EOL interoperability before closing this item.
- [x] Enable native multi-selection typing/deletion, Ctrl-click multiple-selection capability, rectangular virtual space and Alt switching during mouse selection, following upstream initialization. Existing native text-change synchronization updates document contents without refreshing selections. Interactive verification remains pending.
- [x] Multi-select All and Multi-select Next, with each case/whole-word combination, using native occurrence selection. Interactive verification remains pending.
- [x] Undo latest added multi-selection and Skip Current / Go to Next Multi-select, retaining the last occurrence-search flags.
- [ ] **Partial:** virtual-space and column-selection-to-multiple-selection preferences and native navigation are implemented. Complete selection-aware history and verify short-line padding, overlapping ranges, mouse/keyboard conversion and preference changes during an active rectangle.
- [ ] **Partial:** upstream native keyboard/mouse selection semantics and selection direction through model synchronization. The EDIT-compatible adapter sorts selection endpoints.
- [x] Split Lines at viewport width or the last configured column guide (`IDM_EDIT_SPLIT_LINES`, Ctrl+I), matching bundled upstream single-selection behavior. Multiple selections are ignored upstream; rectangular boundary splitting is not an additional requirement. Interactive verification remains pending.
- [ ] **Partial:** rectangular-column sorting and sorting options; existing whole-line sorts are not column sorts.
- [x] Standard Copy/Cut with no selection obeys a persisted Copy/Cut Line preference (enabled by default), publishes line-selection clipboard markers and pastes whole lines before the destination line.
- [ ] **Partial:** binary Copy/Cut/Paste now uses upstream CF_TEXT and Notepad++ Binary Length, preserving embedded NUL and original EOL for UTF-8 text. Arbitrary non-UTF-8 bytes still require a byte-preserving document representation; full interoperability verification remains.
- [ ] **Partial:** language-aware Enter/Tab indentation at every caret; the current multiple-selection path uses native insertion and bypasses the single-selection language indentation handler.
- [ ] **Partial:** upstream multi-caret Delete-at-end-of-line behavior, beyond forwarding basic Scintilla deletion commands.
- [ ] Clipboard History panel, capture policy and insertion actions.
- [ ] Character Panel with code-page selection and character insertion.
- [x] Search selected text/current word on the Internet with a persisted HTTP(S) search-engine template and upstream `$(CURRENT_WORD)` placeholder. Preset picker is available in Preferences and directly through Change Search Engine.
- [ ] Clickable URLs with configurable schemes, underline/full-box behavior and opening actions.
- [ ] Configurable Ctrl+double-click delimiter selection, including multi-line delimiters.
- [ ] **Partial:** configurable word characters used consistently by native navigation, selection, completion and Rust search/edit operations.
- [ ] Smart occurrence highlighting, including case/whole-word options and highlighting the other view.
- [x] Automatic brace-match/bad-brace highlighting and lexer-aware matching/navigation through Scintilla, including highlighted indentation guide, command availability and inclusive brace selection. Interactive verification remains pending.
- [ ] Matching HTML/XML tag highlighting, attribute highlighting and comment/PHP/ASP-zone highlighting.
- [ ] Visible change-history markers/indicators, next/previous change navigation and clear-history command.
- [x] Bookmark margin symbols synchronized with Rust bookmarks, unmodified left-click toggling and right-click Bookmark menu. Source integration exists in `scintilla.rs` and `app.rs`; interactive verification remains pending. Bookmark navigation/edit operations and manual-session bookmarks also exist.
- [ ] **Partial:** preserve native overwrite/insert behavior, drag/drop edits, IME composition and undo grouping across the Rust/native boundary; see verification section.

## 3. Search, Replace, Mark and results

- [ ] One unified modeless Find/Replace/Find in Files/Mark tabbed dialog with shared state, focus, keyboard behavior, sizing and transparency options. Current tabs switch separate owned dialogs.
- [ ] Find/Replace field swap and one-way copy controls, compact/expanded dialog mode, one/two navigation-button modes and the explicit regex “. matches newline” checkbox.
- [ ] Incremental Search bar and its live match/count/navigation behavior.
- [ ] Full Mark tab: match indicators, Mark All, Clear All, purge previous marks, bookmark-line checkbox and persistent options. Existing Bookmark Matching Lines only covers the bookmark subset.
- [ ] Find All in Current Document and Find All in All Opened Documents with result groups.
- [ ] Replace All in All Opened Documents, with confirmation, read-only handling and per-document counts.
- [ ] Replace in Files: confirmation, filters, recursion, encoding preservation, open-buffer policy, cancellation, progress and failure reporting.
- [ ] Find/Replace in Projects, including selection of Project Panels 1/2/3.
- [ ] Find Characters in Range dialog, range bounds, direction and wrap behavior.
- [ ] Style All Occurrences and Style One Token with each of the five indicator styles.
- [ ] Clear one/all styles; jump up/down through each style and the Find Mark style.
- [ ] Copy text belonging to each style, all styles, and Find Mark style.
- [ ] **Partial:** full Boost-compatible regex syntax and replacement behavior; the Rust search engine is not upstream Boost merely because Scintilla's C++ regex code is linked.
- [ ] **Partial:** upstream regex zero-length traversal/replacement rules, Unicode/word boundaries and unsupported escape/backreference/conditional forms.
- [ ] **Partial:** persistent query/replacement/filter/directory histories and all search preferences across launches.
- [ ] **Partial:** Find dialog auto-fill limits, selected-word behavior, default directory selection, Replace-and-stop and result-dialog visibility settings.
- [x] Persisted Find in Files preference to search disk contents instead of dirty open buffers; default continues searching open-buffer snapshots. Interactive comparison and restart verification remain pending.
- [ ] **Partial:** result tree/text with collapsible search/file groups, inline hit highlighting, upstream indentation/colors and newest-first search blocks. Current results are an owner-drawn list.
- [ ] **Partial:** upstream long-line result truncation/hit visibility rules rather than fixed 500-character previews; multiline hit rendering and complete result selection/copy behavior. Upstream also imposes a result-line length limit.
- [ ] Find in These Search Results, including match-line-number option and secondary result panels; close an individual secondary results panel.
- [ ] Results context actions: copy selected pathnames, open selected pathnames, verbatim text copy/select-all, and purge-for-every-search preference. Basic selected-line copy, clear and hide already exist.
- [ ] **Partial:** adjust result positions as documents change and revalidate stale matches before jumping.
- [ ] **Partial:** dock/float/resize the results panel, configurable wrap and fold behavior, and persistence of its layout.
- [ ] **Partial:** cancel long file reads and individual expensive regex operations promptly; current cancellation is between bounded work units/matches.
- [ ] **Partial:** large-file search without loading every file into memory, with responsive progress/error reporting.
- [ ] **Partial:** encoding detection and system ANSI policy shared consistently between file opening and file search.

## 4. Views, tabs, panels and window management

- [ ] A second editor view with vertical/horizontal splitter and independent selection, scroll, zoom, wrap and focus.
- [ ] Move/clone the current document to the other view, sharing document text/history without losing independent view state.
- [ ] Focus Another View, synchronized vertical/horizontal scrolling and zoom synchronization.
- [ ] Move/Open current document in a new instance; cross-window/tab drag-and-drop.
- [ ] **Partial:** Post-It mode has borderless/topmost display, hidden chrome and F12 state restoration. A DPI-scaled top-right Restore button is implemented; upstream floating-control presentation and fullscreen interaction still need verification. Distraction Free Mode now combines fullscreen/Post-It, hides results and restores previous margins; padding is configurable (divisor 3–9, default 4); interaction verification and future docking/two-view integration remain.
- [x] View current file in Firefox, Chrome, Edge and the source's legacy IE route where available; registered executable lookup, legacy Edge fallback and untitled-buffer disablement implemented.
- [x] Toggle spaces/tabs, actual source line endings and wrap symbols; persist these view settings.
- [x] Toggle non-printing characters, control/Unicode EOL characters and all characters in the editor, with persisted settings.
- [ ] Apply the same non-printing/control display settings to the search-results renderer when replacing its owner-drawn list.
- [x] Toggle indentation guides through the upstream menu and persist the setting.
- [x] Hide selected interior lines through View > Hide Lines, with independent boundary markers and left-click reveal in the symbol margin. Merge overlapping/adjacent marked sections and retain the first/last document lines.
- [ ] **Partial:** reconcile hidden-section markers after model-driven edits/undo and deletion of section boundaries. Native text and fold-level changes now queue a coalesced re-hide pass after notification processing. Explicit fold commands and mouse folding now reapply hidden sections; revealing sections retains closed language folds. Tab switches and unchanged-text refreshes now retain sections with revision validation. Upstream reveal artwork and Alt+H shortcut are implemented; verify interaction with hidden bookmark margins, high-DPI margin sizing and nested language folds.
- [x] RTL/LTR commands with mirrored GDI editor layout, reversed horizontal movement/selection bindings, wrap recalculation, per-tab direction and menu check state.
- [x] Manual sessions save/restore per-file RTL direction using the upstream `RTL="yes/no"` attribute; missing attributes restore LTR. Ctrl+Alt+R/L selects direction and Ctrl+Left/Right selection/navigation follows the mirrored direction.
- [ ] **Partial:** results/document-map direction propagation; verify mirrored margin hit targets, mixed-script editing, session restoration and keyboard shortcuts interactively.
- [ ] Document Map with viewport indicator/navigation and document peeking.
- [ ] Document List panel with selection, sorting, configurable columns and context actions.
- [ ] Function List with per-language parser definitions, filtering/sorting, navigation, reload and export.
- [ ] Project Panels 1/2/3: workspace files, projects, virtual folders, file management, relative paths, persistence and search integration.
- [ ] Project context actions: new/open/reload/save/save-as/save-copy workspace; new project/folder; add files/recursive files; rename/remove; modify file path; move up/down; Find in Projects.
- [ ] Folder as Workspace tree: filesystem changes, folder/file actions, filtering, persistence and symlink preference.
- [ ] Workspace-tree context actions: add/remove/remove-all roots, open in editor/default application, copy pathname/filename, Find in Files, Explorer/command prompt/PowerShell here.
- [ ] General panel docking/floating/tabbing, hide/show commands, keyboard focus and layout persistence.
- [ ] Windows dialog: multi-file selection, activate/save/close and sorting.
- [ ] Document List multi-file close/close-others/copy-names/copy-paths and recent-window menu copy-name/copy-path actions.
- [ ] MRU Ctrl+Tab switcher, recent-window dropdown and configurable cycling order.
- [ ] Tab colors 1–5 and reset, pinned tabs, icons and configurable active/inactive visual treatments.
- [x] Persisted tab drag lock, retaining selection and explicit tab movement commands.
- [x] Persisted multiline tab layout, row-aware editor sizing and cross-row mouse reordering.
- [ ] Vertical tab layout and reduced-size tab styling.
- [x] Configurable compact tab label length (0–257), using upstream Windows filename compaction and preserving original filenames.
- [x] Configurable tab close-button visibility and close buttons on inactive tabs, with synchronized click targets and immediate preference redraw. Pin-button behavior remains part of pending pin support.
- [ ] Tab/document-map preview on hover (Document Peeker).
- [x] Configurable double-click tab close and exit when the last document is closed, with persisted settings and the existing save/cancel confirmation flow.
- [x] Optional first-line-based untitled tab names, normalized and checked for duplicate open-tab names; empty/duplicate candidates retain the existing name.
- [ ] **Partial:** upstream tab context menu, including view/instance transfer, pin/color and workspace actions. Basic close/reorder actions already exist.
- [ ] **Partial:** complete toolbar commands, icon styles/sizes, disabled states, colorization and toolbar customization.
- [ ] Menu-bar right-side New/Dropdown/Close shortcuts and their visibility preference.
- [ ] System tray icon and its Activate, New, New and Paste, Open and Close Tray Icon actions.

## 5. Encoding, languages, styles and completion

- [ ] System-ANSI encoding based on the configured/current Windows code page; do not treat Windows-1252 as universal ANSI.
- [ ] Upstream Encode-in/current-buffer reinterpretation commands, distinct from converting the document or opening another file with a forced encoding.
- [ ] Remaining legacy encodings: ISO-8859-1/2/3/4/5/6/7/8/9/13/14/15; OEM 437/720/737/775/850/852/855/857/858/860/861/862/863/865/866/869; Windows-1250/1253/1254/1255/1256/1257/1258; KOI8-R/U; Macintosh Cyrillic; Big5; GB2312; Windows-949; EUC-KR; TIS-620. Individual source IDs are in the appendix.
- [ ] **Partial:** configurable automatic legacy-encoding detection; Unicode BOM/BOM-less UTF-16 detection already exists.
- [ ] Style Configurator: global styles, lexer/token colors, fonts/sizes, bold/italic/underline, overrides, keywords and extension association editing.
- [ ] Theme selection/import/save and upstream `stylers.xml`/theme compatibility; generated built-in colors are not a configurable theme system.
- [ ] Complete User Defined Language editor: keyword groups, comments, operators, delimiters/nesting, numbers, folding rules, case handling and styles.
- [ ] Import/export upstream UDL XML, duplicate/rename/delete definitions and runtime UDL lexing/folding. Current JSON definitions only cover name/extensions/comments/keywords.
- [ ] **Partial:** editable built-in language associations. Built-in menu exclusions and compact/flat layout are implemented; dual-list movement, persistence and menu refresh still require interactive verification.
- [x] Repair incomplete language metadata aliases: the ActionScript (`IDM_LANG_FLASH`), AutoIt (`IDM_LANG_AU3`) and JavaScript (`IDM_LANG_JS`) registry entries currently have empty extension lists. Reconcile their comments/completion metadata with the correct upstream language entries as well as the already-generated syntax profiles.
- [ ] **Partial:** language-specific indentation settings and default inheritance. Global Backspace-unindent and None/Basic/Advanced controls are implemented; per-language overrides and full advanced-indent behavior remain.
- [ ] **Partial:** API-file-backed function completion and parameter calltips with overload previous/next navigation. The current completion workflow is a basic word/keyword chooser.
- [ ] Automatic completion while typing: trigger length, function/word/both modes, brief-list option, ignore numbers and Tab/Enter acceptance controls.
- [ ] Filesystem path completion.
- [ ] Auto-insert configurable bracket/quote pairs, custom pairs and HTML/XML closing tags.
- [ ] **Partial:** verify imported keyword-set mappings and lexer properties for embedded/mixed languages against upstream language setup; generating a profile is not proof of full lexical parity.
- [ ] **Partial:** import/persist upstream `langs.xml`, API files, function-list definitions and language configuration rather than only compiled metadata.

Built-in highlighting and expand/collapse are already implemented. The conversion's new JSON/XML formatter and generic reindent commands are additional features; a full rustfmt/Black integration is **not** counted as a missing built-in Notepad++ feature.

- [x] Persisted scroll-beyond-last-line preference, enabled by default as upstream.
- [x] Persisted caret width/style (0–3 pixels, Block, Block After) and blink interval, including disabled blinking.

- [x] Persisted current-line none/background/frame highlighting and frame width (1–6 pixels).

- [x] Persisted wrapped-line indentation modes: default, aligned and indented, with upstream aligned default.

- [x] Persisted smooth-font option using Scintilla LCD-optimized rendering.
- [x] Persisted right-click-keeps-selection option for the text area, retaining native margin actions.

- [x] Persisted option to disable selected-text drag-and-drop via Scintilla.
- [x] Dynamic/constant line-number margin sizing based on visible/document line count and actual font metrics.

- [x] Persisted bookmark-margin visibility without changing bookmark state or commands.
- [x] Persisted left/right text padding (0–9 pixels) on a dedicated Margins preferences page.

- [x] Persisted fold-margin symbols: None, Simple, Arrow, Circle and Box, preserving fold state.

- [x] Persisted multiple vertical column guides and single-column background mode, with an empty list disabling guides.

- [x] Optional toggle behavior for Fold/Unfold Current Level, persisted and disabled by default.

## 6. Preferences still absent or incomplete

Reference: `notepad_plus_plus/PowerEditor/src/WinControls/Preference/preference.rc`. The current `Windows/src/preferences.rs` persists tab width, tabs/spaces, auto-indent, wrap, toolbar, line-number/status/tab-bar visibility, zoom, whitespace/EOL/non-printing/control display, indentation guides and wrap symbols. A fixed default/native capability is not an implemented configurable preference.

- [ ] **General:** localization selector, dialog font size and menu-right-shortcuts visibility; persist menu visibility changes.
- [ ] **Toolbar:** Fluent/Filled Fluent/standard icon choices, small/large sizes, complete/partial colorization and default/accent/custom color selection.
- [ ] **Partial — Tab Bar:** vertical layout, reduced size, alternate icons, inactive colors, active bar and pin-button visibility. Multiline layout, drag lock, double-click close, exit-last-tab and close-button visibility controls already exist.
- [ ] **Editing 1:** configurable selected foreground color (Style Configurator) and full advanced-scrolling parity. Selection foreground override, virtual space and Disable advanced scrolling toggles are implemented; hovered-control forwarding currently stays within the application UI thread.
- [ ] **Editing 2:** verify column-to-multiedit navigation integration, CRLF appearance/color, non-printing abbreviation/codepoint/color, inclusion of C0/C1/Unicode EOL and prevention of C0 typing.
- [ ] **Dark Mode:** light/dark/follow-system, tone presets, custom control/dialog/text/link/error/edge colors and reset.
- [ ] **Margins/Border/Edge:** border width/no-edge and change history margin/text. Distraction-free padding is implemented with a persisted 3–9 divisor.
- [ ] **New Document:** remaining default ANSI/code-page choices, ANSI-as-UTF8 policy and extra new tab at startup with session restoration. Default EOL, supported encodings, built-in language and content-derived untitled names are implemented.
- [ ] **Default Directory:** follow document/remember/fixed path and folder-drop policy.
- [ ] **Verify — Recent Files History:** startup existence checking, direct-in-File/submenu placement, maximum count and label preferences are implemented. Verify launch/persistence, inaccessible/network paths and menu stability alongside UDL entries.
- [ ] **Verify — Language:** built-in inclusion/exclusion, compact/flat layout and SQL backslash escape preferences are implemented; verify dual-list movement, Cancel/OK, persistence, active SQL recoloring and menu selection checks.
- [x] Global Backspace-unindent preference, persisted and disabled by default, on a dedicated Indentation preferences page.
- [x] Mutually exclusive None / Basic / Advanced auto-indent controls, preserving existing saved auto-indent settings.
- [x] Basic newline indentation searches backward over empty lines for the preceding nonempty line, while retaining whitespace-only lines as indentation sources. None mode skips this search; Advanced retains its language-aware path.
- [ ] **Indentation:** per-language inheritance, width/style, Backspace-unindent overrides and three auto-indent modes.
- [ ] **Highlighting:** style-occurrence case/word rules, matching tags/attributes/zones, smart highlighting, other-view highlighting and reuse of Find settings.
- [ ] **Print:** line numbers, form-feed page breaks, color modes, margins, header/footer text, fonts, size, bold/italic and variable expansion.
- [ ] **Searching:** auto-fill directory/selection/word, max fill length, monospaced dialog font, stay-open-on-results, cross-document Replace All confirmation, Replace-and-stop, one-entry-per-line choice and prefer-disk-content policy.
- [ ] **Backup:** startup session restore, periodic snapshots/interval/path, absent-file retention and no/simple/verbose/custom-directory save backups.
- [ ] **Auto-Completion:** automatic popup/mode/threshold, brief list, calltips, acceptance keys, ignore numbers, bracket/quote/tag insertion and custom pairs.
- [ ] **Multi-Instance & Date:** single/always-multi/session-new-instance mode; per-panel state retention with other instances/`-nosession`.
- [ ] **Delimiter:** custom word-character list and open/close selection delimiters with multi-line option.
- [ ] **Cloud & Link:** alternate/cloud settings location, clickable links, underline/full-box behavior and URI schemes.
- [x] **Search Engine:** Preferences page with Google, DuckDuckGo, Yahoo!, Stack Overflow and custom URL selection; Change Search Engine opens it directly.
- [ ] **Performance:** large-file threshold/restrictions, selective feature disabling, global wrap suppression and ≥2GB warning policy.
- [ ] **Misc:** file-status detection/silent reload/scroll-last-line, MRU switcher, document peeker, tray behavior, rendering mode, mute sounds, encoding autodetection and workspace symlink loading.
- [x] Filename-only window title preference, applied immediately while retaining the modified-document marker; interactive verification remains pending.
- [x] Save All confirmation with Yes / Always yes / No, cancellation and persisted enable/disable preference (enabled by default). Always yes disables future confirmation; Preferences can re-enable it. Interactive verification remains pending.
- [ ] Workspace-file extension preference and recognition, alongside session-file extension handling.
- [ ] General settings XML import/export/migration and compatibility with upstream `config.xml`; retain unknown settings when round-tripping.
- [ ] Shortcut Mapper: main-menu, macro, Run, plugin and Scintilla shortcut categories; editing/removal, conflicts, filtering and persistence.
- [ ] Import style themes and edit/reload popup `contextMenu.xml`.

- [x] Persisted default EOL for new documents (CRLF/LF/CR), including startup and replacement empty buffers.

- [x] Persisted default encoding for new documents across all currently supported Unicode/legacy encodings. Full system-ANSI/code-page coverage remains pending.

- [x] Persisted default built-in language for new/startup/replacement empty documents.

- [x] Optional persisted recent-history file-existence checking at startup, disabled by default.

- [x] Persisted recent-files placement directly in File or in a submenu, including Open All and Clear History.

- [x] Separate recent-file and Post-It Restore command IDs from the user-defined-language range.

- [x] Persisted SQL backslash escape preference with immediate active SQL recoloring.

- [x] Persisted compact/flat Language menu layout with immediate rebuilding.

- [x] Persisted built-in Language menu exclusions without disabling language detection or lexing.

## 7. Macro, Run, tools and printing

- [ ] **Partial:** record/replay navigation, selection and search actions, relevant Scintilla messages and command parameters. Current macros record a whitelist of edit commands plus text diffs.
- [ ] **Partial:** upstream `shortcuts.xml` macro storage/import/export rather than standalone JSON edit-step files.
- [ ] Named saved macros in the Macro menu, shortcut assignment, rename/delete and management UI.
- [ ] Run Macro Multiple Times: select saved macro, repeat until end-of-file and full dialog behavior; fixed repetition count already exists.
- [ ] **Partial:** Run dialog with named commands, save/shortcut controls and persistent command history.
- [ ] **Partial:** remaining Run variables (filename components, application path, current line/column and others defined upstream), quoting/argument handling and upstream launch semantics. Current implementation expands four variables through `cmd.exe`.
- [ ] Saved Run commands in the menu and shortcut manager; validate `shortcuts.xml` command.
- [ ] **Partial:** hash-dialog UI parity and options supported by upstream; the current implementation uses simpler input/file/result dialogs.
- [x] Generate from files for MD5/SHA-1/SHA-256/SHA-512 accepts multiple files and presents their path/hash results together in a selectable, scrollable result dialog, retaining per-file failures.
- [ ] **Partial:** styled printing/WYSIWYG, invert/black-on-white/no-background modes, configurable margins and fonts.
- [ ] Print line numbers, headers/footers and their variables, form-feed page breaks and page-range selection. Current print dialog disables page-number ranges.

## 8. Startup, integration, localization and distribution

- [ ] Single-instance IPC/file forwarding, configurable multi-instance behavior and session-per-instance handling.
- [ ] Command-line switches beyond filenames: `-multiInst`, `-noPlugin`, `-ro`, `-fullReadOnly`, `-fullReadOnlySavingForbidden`, `-nosession`, `-notabbar`, `-systemtray`, `-loadingTime`, `--help`, `-alwaysOnTop`, `-openSession`, `-r`, `-export=functionList`, `-quickPrint`, `-notepadStyleCmdline`, `-openFoldersAsWorkspace`, `-settingsDir=`, `-titleAdd=`, `-udl=`, `-pluginMessage=`, `-monitor`, `-monitoringMode`.
- [ ] Remaining command-line value forms for line/column/position/language and upstream quoting/wildcard/Notepad-compatible argument handling; parse them instead of treating every argument as a filename.
- [ ] Upstream command-line localization override and optional quote/easter-egg/ghost-typing modes (low priority, separate from normal editing).
- [ ] Windows shell registration: Edit with application, file associations/Open With and installer integration.
- [ ] Updater workflow, update policy/channel, updater proxy settings and appropriate rewrite release endpoints.
- [ ] Portable/local/cloud settings-location behavior and configuration migration.
- [ ] Translation/native-language XML loading across menus, dialogs, messages and tooltips; runtime localization selection.
- [ ] **Partial:** high-DPI/per-monitor scaling, dialog font propagation, dark-mode/high-contrast styling and complete consistent UI layout.
- [ ] **Partial:** accessibility integration for editor text, selection, fold state, search results and custom-drawn tabs; verify screen-reader/UI Automation output after replacing EDIT.
- [ ] **Partial:** comprehensive keyboard access, mnemonics and focus traversal for every new panel/dialog.
- [ ] Windows executable branding/version resources, installer/uninstaller, distribution packaging and release/update integration.
- [ ] **Conversion requirement:** implement the macOS frontend; its directory currently contains only `.gitkeep`.
- [ ] **Conversion requirement:** implement the Linux frontend; its directory currently contains only `.gitkeep`.
- [ ] **Conversion requirement:** native file dialogs, clipboard, printing, shortcuts, settings locations and packaging for macOS/Linux while retaining shared document behavior.

## 9. Plugin compatibility — recorded, excluded from implementation scope

The conversion has ABI types, loader/registry/menu plumbing and a subset of host messages; that is not a complete working plugin ecosystem.

- [ ] Discover installed plugins, load enabled DLLs at startup, handle failures and unload safely.
- [ ] Plugins Admin: catalog, installed/available/update lists, install/update/remove, version/platform compatibility and restart flow.
- [ ] Full NPPM host-message behavior and synchronous/reentrant semantics, beyond current query/session/menu/modeless-dialog support.
- [ ] Complete NPPN/Scintilla lifecycle notifications, buffer/view identity and plugin-safe text/history synchronization.
- [ ] Native plugin docking panels and their persistence, toolbar additions, indicators, external lexers and UI integration.
- [ ] Scintilla document ownership/direct-access compatibility with the Rust core; plugin edits must not bypass model/history/save synchronization.
- [ ] Real-plugin compatibility validation, bitness/platform error reporting and disabled/safe-mode management.
- [ ] Cross-platform extension strategy; Windows DLL binary compatibility cannot be assumed on macOS/Linux.

## 10. Verification and compatibility debt — not presumed absent

- [ ] **Verify:** keyboard/mouse/IME input, native context-menu edits, drag/drop, overwrite mode, Unicode/NUL clipboard, read-only and undo grouping with the new Scintilla bridge.
- [ ] **Verify:** every menu command already marked implemented against upstream behavior, including accelerators, enable/check state, selection direction, read-only handling and undo granularity.
- [ ] **Verify:** language profiles, nested/embedded lexers, keyword-set mappings, fold-current/levels and live folding after edits for representative languages.
- [ ] **Verify:** previously written native EDIT-specific checks and assumptions after the Scintilla replacement; do not treat older EDIT evidence as validation of the new renderer.
- [ ] **Verify:** full Unicode/locale sorting, case conversion, word classes, mixed-EOL boundaries and regex compatibility against the bundled source.
- [ ] **Verify:** large-file responsiveness and memory: native text/UTF-16 conversion and model synchronization still copy/scan whole buffers in important paths.
- [ ] **Verify:** save durability and recovery across filesystem types, ACLs, streams, hardlinks/symlinks, external-change races, disk-full and interrupted writes.
- [ ] **Verify:** actual printer output, command launch behavior, tab/window state restoration, high-DPI and accessibility workflows.
- [x] Preserve normal toolbar/tab/status preferences while Post-It or Distraction Free temporarily hides them. Preference reads use the saved normal visibility; applying preferences updates restoration state without revealing hidden chrome.
- [ ] **Verify:** change preferences and exit/relaunch from Post-It and Distraction Free; check restored visibility and mode combinations interactively.
- [x] Reject cached native selection snapshots after background text changes by checking the document revision identity as well as the main selection. Unchanged buffers retain all native ranges; edited buffers use the current model selection.
- [ ] **Partial:** remap all inactive native selection ranges through background edits instead of falling back to the main model selection; verify background edits and undo/tab-switch interactions interactively.
- [ ] **Verify:** the reported Find in Files question-mark rendering and embedded-NUL opening failure against the current Scintilla build. Result rows now escape control characters and native text transfer is length-aware; check Unicode paths/content, BOM-less UTF-16, real NULs, result navigation and save round-trips rather than assuming the old EDIT failure still exists.
- [ ] Reconcile stale claims in `README.md` and `docs/parity.md` with this audit and future implementation evidence; keep completion evidence scoped to what was actually checked.

## 11. Individual upstream main-menu routes still missing or partial

The following appendix is derived from the bundled menu resource and manually filtered for known dispatcher ranges and rewrite aliases. Entries marked **partial alternative exists** must not be counted as completely missing encoding support. The higher-level sections above specify the required behavior. Completed routes are retained as checked entries. High-level entries and appendix routes overlap and must not be summed as independent missing features.

The menu coverage cross-check accounts for every active non-plugin resource command through a production reference, a checklist route, or the documented tab/fold ranges and move-to-start/end aliases. The commented-out ISO-8859-10 menu entry is excluded. A production reference alone does not establish complete behavior; partial implementations are covered in the thematic sections above. Dynamic menus, context actions, preferences and startup features are audited separately from the static main-menu resource.


### File

- [x] File > Open Containing Folder > cmd (`IDM_FILE_OPEN_CMD`).
- [x] File > Open Containing Folder > PowerShell (`IDM_FILE_OPEN_POWERSHELL`).
- [ ] File > Open Containing Folder > Folder as Workspace (`IDM_FILE_CONTAININGFOLDERASWORKSPACE`).
- [ ] File > Open Folder as Workspace... (`IDM_FILE_OPENFOLDERASWORKSPACE`).
- [ ] File > Close Multiple Documents > Close All but Pinned Documents (`IDM_FILE_CLOSEALL_BUT_PINNED`).

### Edit

- [x] Edit > Begin/End Select in Column Mode (`IDM_EDIT_BEGINENDSELECT_COLUMNMODE`).
- [x] Edit > Line Operations > Split Lines (`IDM_EDIT_SPLIT_LINES`).
- [ ] Edit > Auto-Completion > Function Parameters Hint (`IDM_EDIT_FUNCCALLTIP`).
- [ ] Edit > Auto-Completion > Function Parameters Previous Hint (`IDM_EDIT_FUNCCALLTIP_PREVIOUS`).
- [ ] Edit > Auto-Completion > Function Parameters Next Hint (`IDM_EDIT_FUNCCALLTIP_NEXT`).
- [ ] Edit > Auto-Completion > Path Completion (`IDM_EDIT_AUTOCOMPLETE_PATH`).
- [ ] **Partial:** Edit > Paste Special > Copy Binary Content (`IDM_EDIT_COPY_BINARY`). Route and length-delimited clipboard format exist; arbitrary non-UTF-8 byte support remains missing.
- [ ] **Partial:** Edit > Paste Special > Cut Binary Content (`IDM_EDIT_CUT_BINARY`). Route and length-delimited clipboard format exist; arbitrary non-UTF-8 byte support remains missing.
- [ ] **Partial:** Edit > Paste Special > Paste Binary Content (`IDM_EDIT_PASTE_BINARY`). Route and length-delimited clipboard format exist; arbitrary non-UTF-8 byte support remains missing.
- [x] Edit > On Selection > Search on Internet (`IDM_EDIT_SEARCHONINTERNET`).
- [x] Edit > On Selection > Change Search Engine... (`IDM_EDIT_CHANGESEARCHENGINE`) opens the Search Engine Preferences page.
- [x] Edit > Multi-select All > Ignore Case  Whole Word (`IDM_EDIT_MULTISELECTALL`).
- [x] Edit > Multi-select All > Match Case Only (`IDM_EDIT_MULTISELECTALLMATCHCASE`).
- [x] Edit > Multi-select All > Match Whole Word Only (`IDM_EDIT_MULTISELECTALLWHOLEWORD`).
- [x] Edit > Multi-select All > Match Case  Whole Word (`IDM_EDIT_MULTISELECTALLMATCHCASEWHOLEWORD`).
- [x] Edit > Multi-select Next > Ignore Case  Whole Word (`IDM_EDIT_MULTISELECTNEXT`).
- [x] Edit > Multi-select Next > Match Case Only (`IDM_EDIT_MULTISELECTNEXTMATCHCASE`).
- [x] Edit > Multi-select Next > Match Whole Word Only (`IDM_EDIT_MULTISELECTNEXTWHOLEWORD`).
- [x] Edit > Multi-select Next > Match Case  Whole Word (`IDM_EDIT_MULTISELECTNEXTMATCHCASEWHOLEWORD`).
- [x] Edit > Undo the Latest Added Multi-Select (`IDM_EDIT_MULTISELECTUNDO`).
- [x] Edit > Skip Current  Go to Next Multi-select (`IDM_EDIT_MULTISELECTSSKIP`).
- [x] Edit > Column Mode... (`IDM_EDIT_COLUMNMODETIP`). Help route implemented; rectangular editing remains pending.
- [ ] Edit > Column Editor... (`IDM_EDIT_COLUMNMODE`).
- [ ] Edit > Character Panel (`IDM_EDIT_CHAR_PANEL`).
- [ ] Edit > Clipboard History (`IDM_EDIT_CLIPBOARDHISTORY_PANEL`).

### Search

- [ ] Search > Incremental Search (`IDM_SEARCH_FINDINCREMENT`).
- [ ] Search > Mark... (`IDM_SEARCH_MARK`).
- [ ] Search > Change History > Go to Next Change (`IDM_SEARCH_CHANGED_NEXT`).
- [ ] Search > Change History > Go to Previous Change (`IDM_SEARCH_CHANGED_PREV`).
- [ ] Search > Change History > Clear Change History (`IDM_SEARCH_CLEAR_CHANGE_HISTORY`).
- [ ] Search > Style All Occurrences of Token > Using 1st Style (`IDM_SEARCH_MARKALLEXT1`).
- [ ] Search > Style All Occurrences of Token > Using 2nd Style (`IDM_SEARCH_MARKALLEXT2`).
- [ ] Search > Style All Occurrences of Token > Using 3rd Style (`IDM_SEARCH_MARKALLEXT3`).
- [ ] Search > Style All Occurrences of Token > Using 4th Style (`IDM_SEARCH_MARKALLEXT4`).
- [ ] Search > Style All Occurrences of Token > Using 5th Style (`IDM_SEARCH_MARKALLEXT5`).
- [ ] Search > Style One Token > Using 1st Style (`IDM_SEARCH_MARKONEEXT1`).
- [ ] Search > Style One Token > Using 2nd Style (`IDM_SEARCH_MARKONEEXT2`).
- [ ] Search > Style One Token > Using 3rd Style (`IDM_SEARCH_MARKONEEXT3`).
- [ ] Search > Style One Token > Using 4th Style (`IDM_SEARCH_MARKONEEXT4`).
- [ ] Search > Style One Token > Using 5th Style (`IDM_SEARCH_MARKONEEXT5`).
- [ ] Search > Clear Style > Clear 1st Style (`IDM_SEARCH_UNMARKALLEXT1`).
- [ ] Search > Clear Style > Clear 2nd Style (`IDM_SEARCH_UNMARKALLEXT2`).
- [ ] Search > Clear Style > Clear 3rd Style (`IDM_SEARCH_UNMARKALLEXT3`).
- [ ] Search > Clear Style > Clear 4th Style (`IDM_SEARCH_UNMARKALLEXT4`).
- [ ] Search > Clear Style > Clear 5th Style (`IDM_SEARCH_UNMARKALLEXT5`).
- [ ] Search > Clear Style > Clear all Styles (`IDM_SEARCH_CLEARALLMARKS`).
- [ ] Search > Jump Up > 1st Style (`IDM_SEARCH_GOPREVMARKER1`).
- [ ] Search > Jump Up > 2nd Style (`IDM_SEARCH_GOPREVMARKER2`).
- [ ] Search > Jump Up > 3rd Style (`IDM_SEARCH_GOPREVMARKER3`).
- [ ] Search > Jump Up > 4th Style (`IDM_SEARCH_GOPREVMARKER4`).
- [ ] Search > Jump Up > 5th Style (`IDM_SEARCH_GOPREVMARKER5`).
- [ ] Search > Jump Up > Find Mark Style (`IDM_SEARCH_GOPREVMARKER_DEF`).
- [ ] Search > Jump Down > 1st Style (`IDM_SEARCH_GONEXTMARKER1`).
- [ ] Search > Jump Down > 2nd Style (`IDM_SEARCH_GONEXTMARKER2`).
- [ ] Search > Jump Down > 3rd Style (`IDM_SEARCH_GONEXTMARKER3`).
- [ ] Search > Jump Down > 4th Style (`IDM_SEARCH_GONEXTMARKER4`).
- [ ] Search > Jump Down > 5th Style (`IDM_SEARCH_GONEXTMARKER5`).
- [ ] Search > Jump Down > Find Mark Style (`IDM_SEARCH_GONEXTMARKER_DEF`).
- [ ] Search > Copy Styled Text > 1st Style (`IDM_SEARCH_STYLE1TOCLIP`).
- [ ] Search > Copy Styled Text > 2nd Style (`IDM_SEARCH_STYLE2TOCLIP`).
- [ ] Search > Copy Styled Text > 3rd Style (`IDM_SEARCH_STYLE3TOCLIP`).
- [ ] Search > Copy Styled Text > 4th Style (`IDM_SEARCH_STYLE4TOCLIP`).
- [ ] Search > Copy Styled Text > 5th Style (`IDM_SEARCH_STYLE5TOCLIP`).
- [ ] Search > Copy Styled Text > All Styles (`IDM_SEARCH_ALLSTYLESTOCLIP`).
- [ ] Search > Copy Styled Text > Find Mark Style (`IDM_SEARCH_MARKEDTOCLIP`).
- [ ] Search > Find characters in range... (`IDM_SEARCH_FINDCHARINRANGE`).

### View

- [ ] **Partial:** View > Post-It (`IDM_VIEW_POSTIT`, F12) is wired; a visible Restore button is implemented; floating-control presentation and interaction verification remain.
- [ ] **Partial:** View > Distraction Free Mode (`IDM_VIEW_DISTRACTIONFREE`) is wired with default quarter-width margins; padding is configurable (divisor 3–9, default 4); interaction verification and future docking/two-view integration remain.
- [x] View > View Current File in > Firefox (`IDM_VIEW_IN_FIREFOX`).
- [x] View > View Current File in > Chrome (`IDM_VIEW_IN_CHROME`).
- [x] View > View Current File in > Edge (`IDM_VIEW_IN_EDGE`).
- [x] View > View Current File in > IE (`IDM_VIEW_IN_IE`).
- [x] View > Show Symbol > Show Space and Tab (`IDM_VIEW_TAB_SPACE`).
- [x] View > Show Symbol > Show End of Line (`IDM_VIEW_EOL`).
- [x] View > Show Symbol > Show Non-Printing Characters (`IDM_VIEW_NPC`).
- [x] View > Show Symbol > Show Control Characters  Unicode EOL (`IDM_VIEW_NPC_CCUNIEOL`).
- [x] View > Show Symbol > Show All Characters (`IDM_VIEW_ALL_CHARACTERS`).
- [x] View > Show Symbol > Show Indent Guide (`IDM_VIEW_INDENT_GUIDE`). Toggle and persistence implemented.
- [x] View > Show Symbol > Show Wrap Symbol (`IDM_VIEW_WRAP_SYMBOL`).
- [ ] View > Zoom > Synchronize Across Views (`IDM_VIEW_ZOOM_SYNC`).
- [ ] View > Move/Clone Current Document > Move to Other View (`IDM_VIEW_GOTO_ANOTHER_VIEW`).
- [ ] View > Move/Clone Current Document > Clone to Other View (`IDM_VIEW_CLONE_TO_ANOTHER_VIEW`).
- [ ] View > Move/Clone Current Document > Move to New Instance (`IDM_VIEW_GOTO_NEW_INSTANCE`).
- [ ] View > Move/Clone Current Document > Open in New Instance (`IDM_VIEW_LOAD_IN_NEW_INSTANCE`).
- [ ] View > Tab > Apply Color 1 (`IDM_VIEW_TAB_COLOUR_1`).
- [ ] View > Tab > Apply Color 2 (`IDM_VIEW_TAB_COLOUR_2`).
- [ ] View > Tab > Apply Color 3 (`IDM_VIEW_TAB_COLOUR_3`).
- [ ] View > Tab > Apply Color 4 (`IDM_VIEW_TAB_COLOUR_4`).
- [ ] View > Tab > Apply Color 5 (`IDM_VIEW_TAB_COLOUR_5`).
- [ ] View > Tab > Remove Color (`IDM_VIEW_TAB_COLOUR_NONE`).
- [ ] View > Focus on Another View (`IDM_VIEW_SWITCHTO_OTHER_VIEW`).
- [ ] **Partial:** View > Hide Lines (`IDM_VIEW_HIDELINES`) is wired with margin reveal; tab/refresh retention, explicit folding integration, upstream marker artwork and Alt+H are implemented; model-edit/undo reconciliation and interactive parity remain pending.
- [ ] View > Project Panels > Project Panel 1 (`IDM_VIEW_PROJECT_PANEL_1`).
- [ ] View > Project Panels > Project Panel 2 (`IDM_VIEW_PROJECT_PANEL_2`).
- [ ] View > Project Panels > Project Panel 3 (`IDM_VIEW_PROJECT_PANEL_3`).
- [ ] View > Folder as Workspace (`IDM_VIEW_FILEBROWSER`).
- [ ] View > Document Map (`IDM_VIEW_DOC_MAP`).
- [ ] View > Document List (`IDM_VIEW_DOCLIST`).
- [ ] View > Function List (`IDM_VIEW_FUNC_LIST`).
- [ ] View > Synchronize Vertical Scrolling (`IDM_VIEW_SYNSCROLLV`).
- [ ] View > Synchronize Horizontal Scrolling (`IDM_VIEW_SYNSCROLLH`).
- [x] View > Text Direction RTL (`IDM_EDIT_RTL`).
- [x] View > Text Direction LTR (`IDM_EDIT_LTR`).
- [ ] View > Monitoring (tail -f) (`IDM_VIEW_MONITORING`).

### Encoding

- [ ] Encoding > ANSI (`IDM_FORMAT_ANSI`).
- [ ] Encoding > UTF-8 (`IDM_FORMAT_AS_UTF_8`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > UTF-8-BOM (`IDM_FORMAT_UTF_8`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > UTF-16 BE BOM (`IDM_FORMAT_UTF_16BE`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > UTF-16 LE BOM (`IDM_FORMAT_UTF_16LE`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > Character sets > Arabic > ISO 8859-6 (`IDM_FORMAT_ISO_8859_6`).
- [ ] Encoding > Character sets > Arabic > OEM 720 (`IDM_FORMAT_DOS_720`).
- [ ] Encoding > Character sets > Arabic > Windows-1256 (`IDM_FORMAT_WIN_1256`).
- [ ] Encoding > Character sets > Baltic > ISO 8859-4 (`IDM_FORMAT_ISO_8859_4`).
- [ ] Encoding > Character sets > Baltic > ISO 8859-13 (`IDM_FORMAT_ISO_8859_13`).
- [ ] Encoding > Character sets > Baltic > OEM 775 (`IDM_FORMAT_DOS_775`).
- [ ] Encoding > Character sets > Baltic > Windows-1257 (`IDM_FORMAT_WIN_1257`).
- [ ] Encoding > Character sets > Celtic > ISO 8859-14 (`IDM_FORMAT_ISO_8859_14`).
- [ ] Encoding > Character sets > Cyrillic > ISO 8859-5 (`IDM_FORMAT_ISO_8859_5`).
- [ ] Encoding > Character sets > Cyrillic > KOI8-R (`IDM_FORMAT_KOI8R_CYRILLIC`).
- [ ] Encoding > Character sets > Cyrillic > KOI8-U (`IDM_FORMAT_KOI8U_CYRILLIC`).
- [ ] Encoding > Character sets > Cyrillic > Macintosh (`IDM_FORMAT_MAC_CYRILLIC`).
- [ ] Encoding > Character sets > Cyrillic > OEM 855 (`IDM_FORMAT_DOS_855`).
- [ ] Encoding > Character sets > Cyrillic > OEM 866 (`IDM_FORMAT_DOS_866`).
- [ ] Encoding > Character sets > Cyrillic > Windows-1251 (`IDM_FORMAT_WIN_1251`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > Character sets > Central European > OEM 852 (`IDM_FORMAT_DOS_852`).
- [ ] Encoding > Character sets > Central European > Windows-1250 (`IDM_FORMAT_WIN_1250`).
- [ ] Encoding > Character sets > Chinese > Big5 (Traditional) (`IDM_FORMAT_BIG5`).
- [ ] Encoding > Character sets > Chinese > GB2312 (Simplified) (`IDM_FORMAT_GB2312`).
- [ ] Encoding > Character sets > Eastern European > ISO 8859-2 (`IDM_FORMAT_ISO_8859_2`).
- [ ] Encoding > Character sets > Greek > ISO 8859-7 (`IDM_FORMAT_ISO_8859_7`).
- [ ] Encoding > Character sets > Greek > OEM 737 (`IDM_FORMAT_DOS_737`).
- [ ] Encoding > Character sets > Greek > OEM 869 (`IDM_FORMAT_DOS_869`).
- [ ] Encoding > Character sets > Greek > Windows-1253 (`IDM_FORMAT_WIN_1253`).
- [ ] Encoding > Character sets > Hebrew > ISO 8859-8 (`IDM_FORMAT_ISO_8859_8`).
- [ ] Encoding > Character sets > Hebrew > OEM 862 (`IDM_FORMAT_DOS_862`).
- [ ] Encoding > Character sets > Hebrew > Windows-1255 (`IDM_FORMAT_WIN_1255`).
- [ ] Encoding > Character sets > Japanese > Shift-JIS (`IDM_FORMAT_SHIFT_JIS`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > Character sets > Korean > Windows 949 (`IDM_FORMAT_KOREAN_WIN`).
- [ ] Encoding > Character sets > Korean > EUC-KR (`IDM_FORMAT_EUC_KR`).
- [ ] Encoding > Character sets > North European > OEM 861 : Icelandic (`IDM_FORMAT_DOS_861`).
- [ ] Encoding > Character sets > North European > OEM 865 : Nordic (`IDM_FORMAT_DOS_865`).
- [ ] Encoding > Character sets > Thai > TIS-620 (`IDM_FORMAT_TIS_620`).
- [ ] Encoding > Character sets > Turkish > ISO 8859-3 (`IDM_FORMAT_ISO_8859_3`).
- [ ] Encoding > Character sets > Turkish > ISO 8859-9 (`IDM_FORMAT_ISO_8859_9`).
- [ ] Encoding > Character sets > Turkish > OEM 857 (`IDM_FORMAT_DOS_857`).
- [ ] Encoding > Character sets > Turkish > Windows-1254 (`IDM_FORMAT_WIN_1254`).
- [ ] Encoding > Character sets > Western European > ISO 8859-1 (`IDM_FORMAT_ISO_8859_1`).
- [ ] Encoding > Character sets > Western European > ISO 8859-15 (`IDM_FORMAT_ISO_8859_15`).
- [ ] Encoding > Character sets > Western European > OEM 850 (`IDM_FORMAT_DOS_850`).
- [ ] Encoding > Character sets > Western European > OEM 858 (`IDM_FORMAT_DOS_858`).
- [ ] Encoding > Character sets > Western European > OEM 860 : Portuguese (`IDM_FORMAT_DOS_860`).
- [ ] Encoding > Character sets > Western European > OEM 863 : French (`IDM_FORMAT_DOS_863`).
- [ ] Encoding > Character sets > Western European > OEM-US : CP437 (`IDM_FORMAT_DOS_437`).
- [ ] Encoding > Character sets > Western European > Windows-1252 (`IDM_FORMAT_WIN_1252`). **Partial alternative exists:** opening/conversion is supported; upstream current-buffer reinterpretation/menu route remains.
- [ ] Encoding > Character sets > Vietnamese > Windows-1258 (`IDM_FORMAT_WIN_1258`).
- [ ] Encoding > Convert to ANSI (`IDM_FORMAT_CONV2_ANSI`).

### Language

All 94 built-in language choices have production registry entries and Scintilla/Lexilla profiles. The remaining Language gaps are the full Define Your Language editor, UDL XML import/export and runtime custom lexing/folding, configurable associations/menu exclusions, and language-specific completion/indentation behavior detailed in section 5. Do not recreate the built-in Language menu as missing.

### Macro

Basic recording/playback, fixed repeat counts and JSON save/load exist. Named saved-macro menu entries, shortcut management, upstream XML compatibility, repeat-to-end-of-file and full navigation/search/selection recording remain outstanding; see section 7.

### Settings

- [ ] Settings > Style Configurator... (`IDM_LANGSTYLE_CONFIG_DLG`).
- [ ] Settings > Shortcut Mapper... (`IDM_SETTING_SHORTCUT_MAPPER`).
- [ ] Settings > Import > Import style theme(s)... (`IDM_SETTING_IMPORTSTYLETHEMES`).
- [ ] Settings > Edit Popup ContextMenu (`IDM_SETTING_EDITCONTEXTMENU`).

### Run

- [ ] Run > Validate shortcuts.xml (`IDM_EXECUTE_VALIDATE_SHORTCUTSXML`).

### Window

- [ ] Window > Windows... (`IDM_WINDOW_WINDOWS`).
- [ ] Window > Recent Window (`IDM_WINDOW_MRU_FIRST`).

### Help

- [ ] ? > Update Notepad++ (`IDM_UPDATE_NPP`).
- [ ] ? > Set Updater Proxy... (`IDM_CONFUPDATERPROXY`).

### Window dropdown

- [ ] ▼ > Recent Window (`IDM_DROPLIST_LIST`).

## Audit refresh notes — current working tree

- Rechecked all 479 active non-plugin menu command IDs after removing resource comments. There are 156 without a direct dispatcher/language-registry reference; all are covered by the appendix or existing range/alias handling. This is not a count of missing features.
- Reconciled stale thematic entries with implemented new-document defaults, recent-file checking/placement, global indentation controls, SQL escaping and Language menu configuration. Their remaining behavioral and verification gaps stay unchecked.
- The checklist covers main menus, dynamic/context menus, all 23 upstream preference subpages, search/results, editor/model integration, panels, sessions/recovery, language/style/completion, macros/Run/printing, startup, localization and distribution. Plugin gaps remain separately excluded from implementation scope; macOS/Linux frontend gaps remain explicit conversion requirements.
- Checked entries indicate source implementation, not blanket runtime validation. Unchecked **Verify** entries are uncertain compatibility claims rather than confirmed missing features. Historical implementation notes below are evidence of earlier work and may describe superseded limitations.
- Documentation-only review: no application changes, builds or unit tests were performed in this review. This source audit cannot establish every runtime edge case; the verification section records those limits.

## Latest review and coverage map — 2026-10-14

This refresh compares the current working tree with the bundled release. It changes this checklist only; historical implementation-log entries below describe earlier states and are not the current completion status.

| Surface reviewed | Upstream evidence | Conversion evidence / checklist location |
| --- | --- | --- |
| All 479 active non-plugin static menu IDs | `PowerEditor/src/Notepad_plus.rc`, `menuCmdID.h` | `Windows/src/app.rs` production dispatcher/menu builder and `languages.rs`; individual gaps in section 11 |
| Editor operations and selection | `NppCommands.cpp`, `ScintillaComponent/ScintillaEditView.cpp`, bundled Scintilla | `app.rs`, `scintilla.rs`, `projection.rs`, editor-core document/history; section 2 |
| Search dialogs and result workflows | `WinControls/FindReplaceDlg/`, `NppCommands.cpp` | `search_dialog.rs`, `find_files.rs`, editor-core search; section 3 |
| Tabs, modes, panels and sessions | `Notepad_plus.cpp`, `WinControls/`, `Parameters.cpp` | `tabs.rs`, `app.rs`, editor-core workspace/session; sections 1 and 4 |
| Languages, lexer configuration and formatting | `langs.model.xml`, `stylers.model.xml`, `ScintillaComponent/`, UDL dialogs | `languages.rs`, `syntax_data.rs`, `scintilla.rs`, `formatting.rs`; section 5 |
| All 23 upstream preference subpages | `WinControls/Preference/preference.rc` | `preferences.rs` and application bindings; section 6 |
| Macros, Run, printing, startup and platform support | `NppCommands.cpp`, `Parameters.cpp`, `winmain.cpp`, relevant dialogs | `macros.rs`, `printing.rs`, `file_launch.rs`, `main.rs`, platform directories; sections 7–8 |

The static-menu check found 156 IDs without a literal production dispatcher/language-registry reference. All are represented in the checklist or accounted for by fold/unfold level ranges, numbered-tab ranges, and the move-tab-to-start/end aliases. This is a coverage check, **not 156 proven absent features**. Counts of unchecked boxes also overlap between thematic requirements and individual menu routes.

Recent source changes already cover the built-in Language menu, highlighting/folding, native multi-selection commands, Split Lines, and configurable Distraction Free margins. Remaining selection/history integration, full UDL, configurable styles/themes, completion/calltips and UI verification are explicitly left open. No unit tests were written or run for this documentation review.

## Implementation log

### 2026-10-14 — multi-file Open and directory terminals

- Multi-file Open now decodes the native multi-selection buffer, opens in returned order and aggregates individual failures. Save/session/single-file dialogs retain their existing contract. Reference: `CustomFileDialog.cpp` multi-selection/result enumeration.
- Added cmd and PowerShell to Open Containing Folder, with directory-based enablement. Launch passes the working directory separately from command text. cmd uses COMSPEC; PowerShell uses the upstream registry location. Reference: `NppCommands.cpp` corresponding command cases.
- Compiled successfully after each of these three features using `cargo build -p npp-windows --target-dir target/feature-build`. No unit tests added or run. Interactive dialog/terminal launch checks remain part of the verification checklist.

### 2026-10-14 — language metadata aliases

- Restored extensions, comment delimiters and completion keywords for ActionScript, AutoIt and JavaScript from the bundled `langs.model.xml` entries (`actionscript`, `autoit`, `javascript.js`). Built-in detection and comment/completion commands now use those definitions.
- Build passed after this feature with the same feature-build command; no unit tests added or run.

### 2026-10-14 — session folds and horizontal scrolling

- Manual Save Session writes cached per-buffer collapsed fold lines; Load Session restores them after text, language and caret state, including the final active document. Invalid/non-header fold lines are ignored.
- Tab switches and manual sessions now retain horizontal offset and scroll width. Session values use the upstream xOffset/scrollWidth attributes; closed-buffer caches are pruned.
- References: Parameters.cpp session Fold/xOffset/scrollWidth parsing and the bundled Scintilla folding/scroll APIs. Compiled after fold persistence and again after horizontal-scroll persistence with `cargo build -p npp-windows --target-dir target/feature-build`. No unit tests added or run. Interactive session round-trip verification remains pending.

### 2026-10-14 — Show Symbol controls and original line endings

- Added View > Show Symbol switches for spaces/tabs, EOL, indentation guides and wrap symbols, including check states and preferences persistence. Reference: NppCommands.cpp Show Symbol handlers and Scintilla view APIs.
- Removed the old EDIT projection's CRLF normalization so native EOL markers reflect actual LF/CR/CRLF data; UTF-16-to-UTF-8 offset mapping and atomic CRLF edit boundaries remain. Native text is refreshed if normalization during an edit changes the model representation.
- Built after the symbol controls and after the projection change; no unit tests added or run. Existing projection checks assume the old CRLF display and need migration in the deferred testing pass. Interactive mixed-EOL and display verification is still pending. Non-printing/control-character/Show All toggles remain unchecked.

### 2026-10-14 — non-printing, controls and Show All

- Added editor switches for non-printing characters and C0/C1/Unicode EOL using the exact 49/64-entry tables from bundled ScintillaEditView.cpp. Hidden controls use upstream zero-width representations; Error List/Escape Sequence lexer style visibility follows the control switch.
- Show All Characters switches whitespace, EOL, non-printing and control settings together; its check state is the conjunction of all four, matching NppCommands.cpp. All settings persist.
- Built after non-printing display, after control display and after Show All; final Clippy passed with warnings denied. No unit tests added/run. Interactive visual verification and results-panel rendering remain pending.

### 2026-10-14 — multi-file hash generation

- Tools > each hash > Generate from files now uses the multi-file picker and renders ordered digest/path rows together, with individual read errors retained. Results use a read-only multiline control with horizontal/vertical scrolling and text selection for copying.
- Reference: `PowerEditor/src/MISC/md5/md5Dlgs.cpp`, `IDC_HASH_FILEBROWSER_BUTTON`, which enumerates `doOpenMultiFilesDlg()` results and builds hash/filename rows. The conversion shows full paths to disambiguate equal basenames.
- `cargo build -p npp-windows --target-dir target/feature-build` and production Clippy with `-D warnings` passed. No unit tests added or run. Interactive multi-file dialog/copy validation remains pending; broader hash-dialog parity stays unchecked.

### 2026-10-14 — lexer-aware brace matching

- Added automatic matching/unmatched brace styles and indentation-guide highlighting. Matching prioritizes the character before the caret, then checks the character after it, for square/round/curly brackets.
- Go to Matching Brace and Select Matching Braces now use `SCI_BRACEMATCH`, preserving lexer-style boundaries instead of the core plain-text scan. Selection includes both delimiters, navigation updates the preferred caret column, and unavailable commands are disabled.
- References: `Notepad_plus.cpp` findMatchingBracePos/braceMatch and `NppCommands.cpp` matching-brace command cases.
- Build and production Clippy with warnings denied passed using `target/feature-build`. No unit tests added or run. Interactive verification for nested braces, strings/comments, caret movement and folded destinations remains pending.

### 2026-10-14 — View Current File in browsers

- Added Firefox, Chrome, Edge and Internet Explorer menu routes. Named buffers launch their disk path as a quoted argument to the registered executable; untitled buffers disable the commands. Missing installations report an error. Unsaved changes are not implicitly saved, matching upstream.
- Reference: `NppCommands.cpp` browser command cases and App Paths lookup, including the legacy Edge shell fallback. Per-user registration is also checked for user-local browser installations.
- Build and production Clippy with warnings denied passed using `target/feature-build`. No unit tests added or run. Actual browser launches remain an interactive verification item.

### 2026-10-14 — Copy/Cut without a selection

- Added the persisted Preferences checkbox, enabled by default as upstream. Copy/Cut without a selection copies the current line with the document EOL, including an unterminated final line. Cut still uses Rust document replacement/history.
- Publishes the Scintilla/Visual Studio whole-line clipboard tags alongside Unicode and existing lossless NUL data. Ordinary Paste recognizes these tags when there is no selection and inserts at the start of the destination line; selection paste retains normal replacement behavior.
- References: `NppCommands.cpp` Copy/Cut cases, `Parameters.h` default, Scintilla `Editor::CopyLineRange` and Windows clipboard registrations.
- Build and production Clippy with warnings denied passed after integration and after correcting final-line EOL behavior. No unit tests added or run. Interactive clipboard interoperability and undo verification remain pending.

### 2026-10-14 — binary clipboard format integration (partial)

- Added Paste Special > Copy/Cut/Paste Binary Content, using CF_TEXT and the four-byte `Notepad++ Binary Length` payload from `clipboardFormats.h` / `NppCommands.cpp`. Clipboard reads validate lengths before accessing data. Cut changes the document only after publishing clipboard data; Paste retains embedded NUL and original line endings.
- Rust String cannot hold arbitrary invalid UTF-8: those payloads report an error without modifying the document. This limitation remains explicitly unchecked, along with macro/interoperability work; the three routes are not claimed as full binary parity.
- Build and production Clippy with warnings denied passed using `target/feature-build`. No unit tests added or run.

### 2026-10-14 — clipboard macros and Internet search

- Binary Cut/Paste records the resulting edit, including NULs, instead of replaying a clipboard-dependent command. Read-only guards apply to both mutations. Build passed after this correction.
- Added Search on Internet and a persisted custom search URL editor. Uses selected text or the current lexer-defined word, percent-encodes UTF-8 query bytes and expands the upstream CURRENT_WORD placeholder. Only HTTP(S) templates are accepted, with the upstream Google default fallback for invalid stored settings. No browser search was executed during implementation.
- Reference: NppCommands.cpp Search on Internet / Change Search Engine cases. Preset selector/full Preferences page remains unchecked.
- Build and production Clippy passed after adding Internet search. No unit tests added or run; interactive verification remains pending.

### 2026-10-14 — search-engine preset picker

- Change Search Engine now offers Google, DuckDuckGo, Yahoo!, Stack Overflow and Custom in a native keyboard-accessible dropdown, selecting the saved preset automatically. Presets populate and lock the URL field; Custom enables editing. Accepted settings persist through the existing preference storage.
- Reference: `WinControls/Preference/preference.rc` Search Engine controls and `NppCommands.cpp` preset URL mappings. The obsolete Bing enum is not a separate active upstream radio choice. Full Preferences-page integration remains pending.
- Build and production Clippy with warnings denied passed. No unit tests added or run; interactive dialog verification remains pending.

### 2026-10-14 — tab close preferences

- Added persisted Preferences controls for double-click closing and exiting after the last document closes, both off by default. Double-clicking empty tab-strip space still creates a new document. Closing routes through normal save/cancel handling; last-tab exit queues the normal application close event after a successful close.
- References: `Parameters.cpp` doubleClick2Close/quitOnEmpty flags, `TabBar.cpp` TAB_DBCLK2CLOSE handling and `NppIO.cpp` TAB_QUITONEMPTY behavior.
- Compiled after adding double-click close and again after last-tab exit. Final production Clippy passed with warnings denied. No unit tests added or run; interactive cancellation/last-tab checks remain pending.

### 2026-10-14 — tab drag lock and cancellation

- Added a persisted Lock tab dragging checkbox. The native tab handler skips mouse capture/drag tracking when locked, and the app rejects any already-queued mouse reorder event. Explicit keyboard/menu movement is unaffected.
- Capture loss, cancellation and double-click clear pending drag state; enabling the lock also cancels an active drag.
- Reference: `TabBar.cpp` TAB_DRAGNDROP checks and `Parameters.h` default enabled dragging. Build passed after the preference feature and after cancellation cleanup; final production Clippy passed. No unit tests added or run. Interactive drag/cancel verification remains pending.

### 2026-10-14 — multiline tabs

- Added the persisted multiline tab preference using TCS_MULTILINE, following the bundled TabBar.cpp style handling. Layout sets the current width, measures native tab rows/item height and moves the editor below all rows. Opening/closing/renaming tabs and resizing the window recalculate the layout.
- Drag threshold now checks both axes so tabs can move between rows; drag lock remains effective.
- Build passed after multiline layout and again after cross-row drag support. Final production Clippy passed with warnings denied. No unit tests added or run. Interactive row wrapping, resize and drag verification remains pending.

### 2026-10-14 — tab close-button visibility

- Added persisted close-button and inactive-tab-button preferences. Text labels and hit testing use the same visibility rules; hidden buttons no longer leave active close targets. Defaults match upstream enabled close buttons and disabled inactive-tab buttons. Middle-click and configured double-click remain separate close gestures.
- Applying preferences redraws tab labels immediately. Reduced checkbox row spacing keeps the expanded Preferences dialog to 484 client pixels high.
- Reference: TabBar.cpp TAB_CLOSEBUTTON/TAB_INACTIVETABSHOWBUTTON handling. Compiled after the feature and after immediate-redraw/layout fixes; final production Clippy passed. No unit tests added or run. Interactive tab hit-target and preference-layout verification remains pending.

### 2026-10-14 — compact tab labels

- Added a persisted Tab label limit control (0–257, zero disables compaction). Labels use PathCompactPathExW with the same buffer-size convention as upstream; failures or invalid Unicode output retain the original label. The document path, save names and title remain unchanged.
- References: `ScintillaComponent/Buffer.cpp` compact filename handling, `Parameters.cpp` range clamping and `NppConstants.h` maximum. Reduced-size tab styling and vertical layout are still separate pending features.
- Build and production Clippy with warnings denied passed. No unit tests added or run; interactive long-name/Unicode/multiline-layout verification remains pending.

### 2026-10-14 — content-derived untitled names

- Added a persisted preference (off by default) and shared-workspace naming support. First-line text is trimmed, filename-invalid/control characters removed and length capped at 63 UTF-16 units without splitting Unicode scalars. Empty or duplicate candidates retain the previous name. Saved documents retain their filenames.
- Native edits and document refresh/undo paths update names; the workspace name remains stable across tab movement. References: NppIO.cpp useFirstLineAsTabName, Buffer.cpp normalizeTabName, and the 64-unit langNameLenMax definition.
- Build and production Clippy passed. No unit tests added or run. Interactive naming/undo/session verification remains pending; explicit untitled rename and snapshot integration are not implemented by this feature.

### 2026-10-14 — persistent closed-file history

- Added atomic recent-files.json storage in the application settings directory. Startup restores newest-first entries with explicit encoding, deduplication and lossless UTF-16 Windows paths; invalid entries are skipped. Missing files remain listed so users can retry when available.
- Successful closes persist immediately (including partially completed Close All), while reopening/saving removes corresponding recent entries. Hidden development/test app instances do not read/write user history. Restore Last Closed File retains its existing strict encoding behavior.
- Reference: Parameters.cpp History/File loading and recent-list persistence. The configurable full history submenu remains pending; this change persists the existing ten-entry closed-file stack.
- Build and production Clippy passed. No unit tests added or run. Interactive restart and unavailable-file verification remains pending.

### 2026-10-14 — Recent Files submenu

- Added a dynamic File > Recent Files submenu with numbered entries, explicit-encoding reopen, Open All and Clear History. Empty actions are disabled. Open All snapshots the list before opening oldest-to-newest and aggregates failures; failed entries remain in the history.
- Labels escape menu ampersands and display ordinary drive/UNC paths without changing stored paths. Clear History persists immediately.
- Reference: NppCommands.cpp IDM_OPEN_ALL_RECENT_FILE / IDM_CLEAN_RECENT_FILE_LIST. Build and production Clippy passed for the feature; build passed again after UNC display correction. No unit tests added or run; interactive menu/restart checks remain pending.

### 2026-10-14 — configurable history limit

- Added persisted Recent files count, validated at 0–30 per upstream NB_MAX_LRF_FILE. Shared workspace restoration/close collection respects the limit, including zero. Changing preferences trims and persists history and refreshes the submenu immediately.
- Preferences now load before history so startup does not prematurely discard entries above the old fixed limit. Expanded dynamic menu command range to thirty entries and limited numeric mnemonics to the first nine.
- References: NppConstants.h NB_MAX_LRF_FILE and Parameters.cpp History nbMaxFile clamping. Build and production Clippy passed; build repeated after menu-refresh changes. No unit tests added or run. Interactive setting/restart verification remains pending.

### 2026-10-14 — recent-file label display

- Added a persisted recent-label setting: -1 displays full paths, 0 displays filenames, 1–259 compacts paths. Validation follows upstream constants; labels retain menu ampersand escaping and the original reopen target.
- References: NppConstants.h RECENTFILES_SHOWFULLPATH/SHOWONLYFILENAME/NB_MAX_LRF_CUSTOMLENGTH and Common.cpp menu-label PathCompactPathEx usage. Settings immediately rebuild the recent menu.
- Build and production Clippy passed. No unit tests added or run. Interactive preference/menu display verification remains pending.

### 2026-10-14 — Preferences navigation and tab-width consistency

- Replaced the growing single-page control list with a category list and General, Editing, Tab Bar and Recent Files History pages. Existing controls remain alive while switching, preserving uncommitted edits; OK/Cancel remains shared. The dialog is now 640×350 client pixels rather than a tall list.
- Reference: upstream Preference dialog category/page organization. This organizes implemented settings only; the many unimplemented upstream settings remain unchecked.
- Corrected tab-width validation to accept the same 1–32 range advertised by the dialog. Custom widths clear the 2/4/8 preset radio selection instead of incorrectly selecting 8.
- Compiled after the tab-width fix and after paged Preferences; final production Clippy passed. No unit tests added or run. Interactive keyboard navigation, page layout and preference validation remain pending.

### 2026-10-14 — Search Engine Preferences page

- Integrated presets and custom URL editing into the shared Preferences dialog. Change Search Engine opens that page directly, matching NppCommands.cpp's preference-page routing. OK saves alongside other preferences; Cancel leaves application settings unchanged. Invalid HTTP(S) templates select the relevant page and keep the dialog open.
- Reference: preferenceDlg.cpp Search Engine page registration and NppCommands.cpp IDM_EDIT_CHANGESEARCHENGINE.
- Build and production Clippy passed. No unit tests added or run. Interactive preset/custom switching and cancellation verification remains pending.

### 2026-10-14 — scroll and caret preferences

- Editing now exposes scroll beyond last line, enabled by default like Parameters.h, via SCI_SETENDATLASTLINE.
- Added saved caret width/style and blink interval controls. Supports line widths 0–3, block, block-after and zero-period nonblinking caret. Defaults use upstream 1-pixel width and 600ms blinking. Changes apply immediately on accepting Preferences.
- References: Notepad_plus.cpp scroll setup, NppBigSwitch.cpp NPPM_INTERNAL_SETCARETWIDTH and preferenceDlg.cpp caret options. Compiled after scrolling and after caret controls; final production Clippy passed. No unit tests added or run. Interactive rendering verification remains pending.

### 2026-10-14 — current-line display

- Added a Current Line preferences page with none/background/frame modes and 1–6 pixel frame width, persisted and applied to Scintilla. Defaults match upstream background highlighting and one-pixel frame width.
- Corrected the category list to include the previously implemented Search Engine page; both it and Current Line are selectable from Settings.
- Reference: Parameters.h current-line defaults and NppBigSwitch.cpp NPPM_INTERNAL_HILITECURRENTLINE frame/visibility logic. Build and production Clippy passed. No unit tests added or run. Interactive visual verification remains pending.

### 2026-10-14 — wrapped-line indentation

- Added the three upstream wrap-indent modes to Editing preferences, persisted and applied through SCI_SETWRAPINDENTMODE. Aligned is the default, matching Parameters.h; mode changes do not alter document content or the Word Wrap toggle.
- Reference: NppBigSwitch.cpp NPPM_INTERNAL_LWDEF/LWALIGN/LWINDENT and ScintillaEditView::setWrapMode.
- Build and production Clippy passed. No unit tests added or run. Interactive mixed-indentation/resize verification remains pending.

### 2026-10-14 — named preference choices and validation navigation

- Replaced numeric mode codes with native dropdown choices for caret style, current-line highlight and wrap indentation. Existing saved values remain compatible. Uses the named choices from upstream preferenceDlg.cpp; caret includes Invisible, 1–3 pixels, Block and Block After.
- Validation of editable numeric settings now selects the containing category and focuses the invalid field before reporting the error, rather than leaving users on a different page.
- Compiled after dropdown conversion and after validation navigation; final production Clippy passed. No unit tests added or run. Interactive keyboard/dropdown verification remains pending.

### 2026-10-14 — font smoothing and right-click selection

- Added saved smooth-font rendering (off by default) using the upstream default/LCD-optimized Scintilla quality mapping.
- Added a Mouse page with right-click selection retention. It suppresses caret relocation only inside the editing area, using upstream margin-position calculation; margin right-click notifications remain intact.
- References: NppBigSwitch.cpp NPPM_SETSMOOTHFONT, Parameters.h defaults and ScintillaEditView.cpp WM_RBUTTONDOWN handling.
- Compiled after smoothing and after selection retention; final build and production Clippy passed. No unit tests added or run. Interactive rendering/context-menu verification remains pending.

### 2026-10-14 — text dragging and line-number width

- Added a persisted selected-text drag disable option using the same SCI_SETDRAGDROPENABLED mapping as NppBigSwitch.cpp. Dragging remains enabled by default.
- Replaced fixed 52-pixel line-number width with font-measured sizing. Dynamic mode measures the last visible document line with a three-digit minimum; constant mode measures total lines with a four-digit minimum, matching ScintillaEditView::updateLineNumberWidth. Scroll/update notifications and layout refresh update the margin.
- Compiled after each feature; final production Clippy passed. No unit tests added or run. Interactive drag suppression, folded-line scrolling and zoom verification remain pending.

### 2026-10-14 — bookmark margin and text padding

- Added bookmark-margin visibility, enabled by default. Hidden markers retain Rust bookmark state and all bookmark commands.
- Added a Margins page grouping line-number visibility/sizing, bookmark visibility and independent left/right text padding (0–9 pixels), matching Parameters.h limits/defaults and NppBigSwitch.cpp margin setters.
- Compiled after bookmark visibility and after padding/page integration; final production Clippy passed. No unit tests added or run. Interactive gutter/padding verification remains pending.

### 2026-10-14 — fold-margin symbol styles

- Added a named dropdown to Margins for None, Simple, Arrow, Circle and Box. All seven fold marker states use the bundled ScintillaEditView marker table; None hides the margin while keeping fold commands and stored collapsed state available. Box remains the default.
- Reference: ScintillaEditView.cpp _markersArray and ScintillaEditView.h set-fold-style behavior.
- Build and production Clippy passed. No unit tests added or run. Interactive marker rendering and hidden-margin folding verification remain pending.

### 2026-10-14 — column guides

- Added a Column Guides preferences page with validated space-separated columns (0–8192), duplicate removal and optional background mode. Empty lists disable the guides. One column can shade the background; multiple columns use line guides, following upstream mode selection.
- Reconfiguring clears old guides before applying new ones; settings persist and do not alter document text. Uses the existing default grey until full Style Configurator support is implemented.
- Reference: NppBigSwitch.cpp multi-edge refresh, column limit and EDGE_BACKGROUND/EDGE_MULTILINE selection.
- Build and production Clippy passed. No unit tests added or run. Interactive guide rendering and restart verification remain pending.

### 2026-10-14 — toggle current-fold commands

- Added the persisted toggle-fold-command option to Margins. When enabled, both current-level commands toggle the applicable fold header; disabled keeps explicit collapse/expand behavior. All/numbered-level commands are unchanged.
- Reference: NppCommands.cpp IDM_VIEW_FOLD_CURRENT/UNFOLD_CURRENT and Parameters.h _enableFoldCmdToggable default.
- Build and production Clippy passed. No unit tests added or run. Interactive nested-fold/current-line verification remains pending.


## Audit refresh — 2026-10-14

This refresh changes the checklist only. Existing implementation changes in the working tree are retained. No unit tests were written or run.

- Re-extracted the active main-menu resource after removing comments: **479 distinct non-plugin command IDs**. Compared these against the production command dispatcher and built-in language registry, excluding the Rust test module. **174** have no literal dispatcher/registry reference; this is a source-navigation metric, not a missing-feature count.
- Every command without a literal reference is represented in the appendix, except the implemented numbered-tab/fold-level ranges and the two documented tab-to-start/end aliases. No unexplained static main-menu route was found outside the checklist.
- Rechecked upstream preference pages against the conversion's preference fields and application wiring. Removed stale missing-feature claims for completed tab/history/content-name settings and the filename-only title option. Save All confirmation was still missing at audit time; see the subsequent implementation log.
- Explicitly marked binary clipboard routes as partial, because they are wired but the UTF-8 document model rejects arbitrary binary bytes.
- Confirmed startup still treats each argument as a filename and that macOS/Linux directories contain only placeholders.
- Main menus alone do not cover feature parity: sections 1–10 also cover context menus, preferences, panels, startup, recovery, file formats, native editor integration and verification debt. Plugin gaps are recorded separately and excluded from the earlier non-plugin implementation scope.
- This is a source audit against the bundled version, not an interactive certification of every feature or a comparison against a newer online release. Checked entries mean source integration exists; outstanding interactive checks remain explicit. Overlapping appendix and thematic entries must not be added together as a feature total.


### 2026-10-14 — Save All confirmation

- Compared upstream `NppIO.cpp::fileSaveAllConfirm`, `Notepad_plus.cpp::doSaveAll` and `IDD_DOSAVEALLBOX`: Yes saves, Always yes saves and disables the preference, No/close cancels.
- Added the same choices using a native modal task dialog and a persisted General preference, enabled by default. Dialog creation failure reports an error without saving. Existing per-file save/error handling remains in use.
- Validation: `cargo fmt --all`, `cargo build -p npp-windows --target-dir target/feature-build` and `cargo clippy -p npp-windows --target-dir target/feature-build -- -D warnings` passed. No unit tests added or run. Interactive dialog and restart verification remains pending.


### 2026-10-14 — RTL/LTR editor direction

- Followed `ScintillaEditView::changeTextDirection` and the RTL/LTR dispatcher in upstream `NppCommands.cpp`: use `WS_EX_LAYOUTRTL`, reverse horizontal Scintilla key bindings and toggle/restore wrapping to recalculate mirrored text.
- Added both View menu routes, active-direction checks and independent in-memory direction per buffer. Closed-buffer direction entries are discarded. The current renderer uses GDI; future DirectWrite configuration must respect upstream RTL restrictions.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Interactive mirrored input/margins and session persistence remain outstanding.


### 2026-10-14 — RTL session state and keyboard integration

- Matched upstream `Parameters.cpp` session `RTL` attribute parsing/writing and Ctrl+Alt+R/L defaults. Manual session load restores direction before refreshing each document; saving writes the current per-buffer direction.
- Corrected Rust-handled Ctrl+Left/Right and Ctrl+Shift+Left/Right navigation to respect RTL, since application accelerators intercept those keys before Scintilla receives them.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Native interaction and session round-trip verification remain pending.


### 2026-10-14 — Column Mode help

- Added the upstream Edit > Column Mode help route, referencing `NppCommands.cpp::IDM_EDIT_COLUMNMODETIP`. Upstream assigns no default shortcut.
- Documented Alt-drag and Alt+Shift+arrow selection gestures, with explicit conversion limitations for the missing Begin/End command and rectangular editing/model synchronization. This does not mark column editing complete.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Interactive verification remains pending.


### 2026-10-14 — Begin/End column selection

- Followed upstream `ScintillaEditView::beginOrEndSelect` and `NppCommands.cpp`: share the view anchor with ordinary Begin/End Select, check the pending mode and disable the other mode. The existing edit-tracked anchor handles intervening edits.
- Completing column selection sets native rectangular endpoints without resetting the editor. Uses the actual native caret rather than the sorted EDIT compatibility endpoint; maps source EOL positions into Scintilla positions. Updated Column Mode help.
- Validation: formatting, compilation after implementation and after caret correction, and Clippy with warnings denied passed. No unit tests added or run. Rectangular copy/paste/undo and tab-switch preservation remain outstanding; interactive verification is pending.


### 2026-10-14 — native selection state across tabs

- Reviewed upstream `ScintillaEditView::saveCurrentPos` / `restoreCurrentPosPreStep` and bundled Scintilla `Selection::ToString`. Capture native selections per displayed buffer and restore after switching document contents, before restoring scroll.
- Serialization retains rectangle/thin/line modes, main range, multiple ranges, direction and virtual space. Closed-buffer state is discarded; changed model selection prevents stale restoration over explicit navigation.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Interactive tab-switch verification remains pending; this does not complete rectangular/multiple-selection editing, undo or clipboard integration.

### 2026-10-14 — multi-selection menu commands

- Implemented eight All/Next case/whole-word variants, Undo Latest Added and Skip Current/Next from upstream `NppCommands.cpp`, using Scintilla target/search/selection commands. Enable native multiple selection before occurrence operations, expand the word for Select All and avoid an empty-word operation. Skip retains the preceding search flags.
- Extended generated Scintilla bindings to include upstream `SCFIND_` constants after the initial build identified missing definitions.
- Validation: final feature-build compilation, formatting and Clippy with warnings denied passed. No unit tests added or run. Interactive occurrence-selection verification remains pending; full multi-caret editing, clipboard, model/history and macro integration remain unchecked.


### 2026-10-14 — native multiple-selection editing setup

- Referenced upstream `Notepad_plus.cpp` editor initialization. Enabled additional-selection typing/deletion, multiple selection, rectangular virtual space and stream-to-rectangle mouse switching.
- Reviewed `Event::NativeChanged` / `Event::Changed`: native edits are synchronized through the existing projection diff without a normal editor refresh, retaining native selections when text projection remains unchanged. This is source evidence only, not interactive verification.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Enter/Tab, clipboard operations, selection-aware undo/redo, preferences and full model/macro integration remain outstanding.


### 2026-10-14 — multiple/rectangular Copy and Cut

- Referenced bundled Scintilla `CopySelectionRange`, `GetSelText`, `ClearSelection` and the Windows `MSDEVColumnSelect` format. Native selected text is copied through the existing NUL-preserving clipboard writer, with rectangular metadata where appropriate.
- Cut computes one model replacement across the selected ranges, retaining intervening text and mixed line endings, then clears the native ranges. Empty multiple-carets do not accidentally delete adjacent characters. Macro recording captures the actual text change.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Interactive clipboard/undo verification remains pending; rectangular paste and full selection-history restoration remain incomplete.


### 2026-10-14 — rectangular paste

- Referenced bundled `ScintillaWin::Paste`, `Editor::ReplaceRectangular` and `InsertPasteShape`. Detect `MSDEVColumnSelect` and Borland block type 2; read the existing length-delimited UTF-8 clipboard format and invoke native rectangular insertion without NUL truncation.
- Set native EOL mode from the document before inserting, suppress intermediate model notifications and synchronize the resulting native text in one transaction. Added exact projection synchronization so untouched mixed line endings between pasted rows are retained. Macro recording captures the resulting text edit.
- Validation: formatting, feature-build compilation and Clippy with warnings denied passed. No unit tests added or run. Native clipboard interoperability, padding and undo interaction need interactive verification. Ordinary multi-caret paste and full selection-history restoration remain outstanding.


### 2026-10-14 — clipboard routing correction and multi-caret paste

- Source inspection found the prior multiple-selection Copy/Cut and rectangular Paste branches had been inserted under Internet Search, making them unreachable from the clipboard commands. Moved the code into the actual Copy/Cut/Paste handler. The prior builds proved compilation only; those earlier implementation entries did not establish reachable behavior until this correction.
- Added ordinary text paste at each native selection, replacing ranges from end to start, realizing virtual-space padding, preserving NUL and document EOL policy, and collapsing each range at its inserted text. Synchronize all edits into one model transaction and macro text change.
- Validation: compiled after the routing correction and again after multi-caret paste; formatting and Clippy with warnings denied passed. A source-scope check confirms clipboard operations are in the clipboard handler and absent from Internet Search. No unit tests added or run. Interactive verification, selection-aware undo and overlapping-selection edge cases remain pending.


### 2026-10-14 — interactive multi-selection verification

- Used the computer-use skill to launch `target/feature-build/debug/npp-windows.exe` in a separate window (11667582), leaving existing debug/release instances alone. Two unsaved scratch tabs remain in that verification instance.
- Verified through native menus that Select All with case/whole-word matching selects both `alpha` occurrences in `alpha beta alpha`. Ctrl+C and paste into a new tab produced two `alpha` lines; returning with Ctrl+1 visibly restored both selections.
- Pasting back into those selections failed: observed `alpha beta\r\nalpha` (18 bytes), not two copies of the 12-byte clipboard string replacing the two words. Reopened the multi-paste checklist item as a confirmed defect. No new code or unit tests in this verification pass.
- Pending next action: diagnose native command routing/selection replacement and queued model events for this concrete reproducer, fix it, compile, and rerun the UI scenario.


### 2026-10-14 — clipboard keyboard routing and UI regression check

- Found Ctrl+C/X/V absent from application accelerators: native Scintilla key handling bypassed the Rust clipboard routes. Added all three accelerators so keyboard and menus share the same handlers.
- Built `target/clipboard-build/debug/npp-windows.exe` because the previous verification build remains running. In the new native UI, selected both `alpha` occurrences, copied and pasted with Ctrl+C/V: the app inserted the full 14-byte payload at both ranges, producing 34 bytes. Ctrl+Z restored the original 16-byte `alpha beta alpha` in one step. Undo restored only one range, confirming selection-history work is still needed.
- The original reported Ctrl+V routing defect is corrected, but the broader clipboard parity item remains open: bundled `CopySelectionRange` treats every multi-range copy as column-shaped and includes a trailing EOL; the custom writer currently marks only geometric rectangles. The earlier 12-byte native clipboard payload and current 14-byte custom payload therefore do not represent the same paste shape. Resolve shape interoperability before closing that reproducer's end-to-end parity requirement.
- Formatting and compilation passed. No unit tests added or run. Scratch verification window 986292 remains open; older user instances were untouched.


### 2026-10-14 — upstream clipboard shapes and alternate shortcuts

- Rechecked `Editor::CopySelectionRange`, `ScintillaWin::CopyToClipboard` and upstream `Parameters.cpp` shortcut defaults. Multi-range copies (including stream selections) carry the column marker; one-range copies do not. Added the Borland block-type byte alongside `MSDEVColumnSelect`. This corrects the earlier assumption that nonrectangular multiple selections must produce stream clipboard data.
- Routed Ctrl+Insert, Shift+Insert and Shift+Delete through app Copy/Paste/Cut, matching the Ctrl+C/V/X routing correction.
- Compiled after the metadata feature and again after alternate shortcuts using `target/column-build`; formatting passed. No unit tests added or run. UI interoperability and selection-aware undo remain pending; prior expected stream-paste output is no longer used as the upstream oracle.


### 2026-10-14 — native edit EOL preservation and multi-caret Enter/Tab

- Native text synchronization now uses exact source text instead of normalizing the entire changed span. This prevents multi-caret edits from converting untouched mixed line endings between carets. The legacy conversion adapter remains only for existing test code; no tests were added or run.
- Match upstream buffer-EOL setup (`ScintillaEditView` buffer-change handling): refresh sets Scintilla EOL mode from the active document, including tab switches and EOL conversion commands. Explicit paste still converts inserted text according to document policy.
- Route Enter/Tab through native Scintilla when more than one selection exists, preserving multiple carets rather than invoking the single-range Rust commands. Language-aware auto-indent at multiple carets, native grouping and selection-aware undo remain incomplete.
- Validation: formatted and compiled after each change with `target/column-build`; Clippy with warnings denied passed. Mixed-EOL typing and multi-caret Enter/Tab still require interactive verification.


### 2026-10-14 — multi-edit preferences

- Added a persisted Multi-edit page: multiple-selection enablement (default on), column-to-multi-edit (default on) and virtual space (default off). Turning multiple selection off unchecks/disables column conversion. Occurrence commands no longer silently override disabled multiple selection.
- Referenced upstream `NPPM_INTERNAL_SETMULTISELECTION`, virtual-space setup and `ScintillaEditView` key handling. Virtual space uses rectangular-only flags by default and adds accessible/no-wrap flags when enabled. Native rectangle navigation/Return/Backspace converts to stream carets with the upstream double mode-set; Escape collapses to the main caret.
- Partial: application accelerator navigation (Home/End and word movement) still bypasses this native key path and needs multi-selection-aware integration. Full column-edit behavior is not marked complete.
- Validation: formatting, compilation after implementation and checkbox dependency wiring, and Clippy with warnings denied passed. No unit tests added or run. Interactive preference/restart and navigation verification remain pending.


### 2026-10-14 — navigation with multiple selections

- Mapped the application's Home/End, document-boundary, word, word-part and paragraph accelerator commands to their bundled Scintilla counterparts when multiple selections, a rectangle or accessible virtual space is active. This avoids converting native selection sets into a single Rust range during navigation.
- Applied upstream column-to-multi-edit mode switching before accelerator navigation; word-left/right retains RTL reversal. Ordinary single-range navigation continues through the existing core implementation. Referenced upstream `Parameters.cpp` Scintilla key mappings and `ScintillaEditView` column conversion.
- Validation: formatting, `target/column-build` compilation and Clippy with warnings denied passed. No unit tests added or run. Native navigation/virtual-space interaction needs UI verification; selection-aware history, multi-caret auto-indent and macro navigation recording remain incomplete.


### 2026-10-14 — multiple-selection deletion commands

- Route Delete, Delete Word Left/Right and Delete to Line Start/End through bundled Scintilla when multiple selections or a rectangle are active, following the upstream Scintilla command mappings in `Parameters.cpp`.
- Suppress intermediate notifications, apply the resulting exact text as one model transaction, retain native carets and record the text change for macros. Read-only checks run before this branch.
- Validation: formatting, `target/column-build` compilation and Clippy with warnings denied passed. No unit tests added or run. Interactive deletion/undo checks remain pending. Upstream's additional multi-caret EOL-deletion handling and full selection-aware history are still missing.


### 2026-10-14 — multiple-selection status counts

- Referenced upstream `Notepad_plus::updateStatusBar`, `getSelectedCharsAndLinesCount`, `getUnicodeSelectedLength` and `getSelectionLinesRange`. Status reports all native selection ranges rather than only the core's main range.
- Stream selections show range count, summed Unicode characters and distinct touched lines; selections ending at the next line's start exclude that following line. More than 99 ranges show an ellipsis for line count, matching the upstream performance limit. Rectangles show rows, maximum character width, equality/unequal-width arrow and total characters.
- Validation: formatting, `target/column-build` compilation and Clippy with warnings denied passed. No unit tests added or run. Native UI verification of status counts, virtual columns and large selection sets remains pending; this does not complete selection-aware history.


### 2026-10-14 — Split Lines

- Implemented the missing menu command and Ctrl+I accelerator using upstream `NppCommands.cpp::IDM_EDIT_SPLIT_LINES` and native `SCI_LINESSPLIT`. Expand a single selection to whole touched lines, excluding an unselected next line; use viewport width without guides or the final guide column plus one character width. Multiple selections are ignored, as in bundled upstream.
- Uses the native-edit synchronization path for one exact-text model transaction and macro text-change recording; read-only checks apply. Corrected the checklist's earlier assumption about rectangular-boundary splitting.
- Validation: formatting, compilation and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run. Interactive wrapping/guide/undo verification remains pending.


### 2026-10-14 — Post-It mode (partial)

- Followed upstream `Notepad_plus::postItToggle`: save frame/chrome/topmost state, hide menu/toolbar/tabs/status, remove the window frame and force topmost; F12 toggles back to the saved state. Added View menu route and upstream F12 shortcut.
- Restore normal frame information through the existing fullscreen state when leaving Post-It while fullscreen. Fullscreen/Post-It combinations still require interactive validation. The floating restore button is not implemented, so Post-It remains partial.
- Validation: formatting, `target/column-build` compilation and Clippy with warnings denied passed. No unit tests added or run.


### 2026-10-14 — Post-It restore control

- Added a native keyboard-focusable Restore (F12) button above the editor at the top-right, DPI-scaled and repositioned during layout. It is shown only in Post-It and dispatches the same restoration command as F12. Creation failure leaves the mode unchanged.
- Corrected leaving fullscreen while Post-It remains active so the normal frame is not restored prematurely.
- The button is a child overlay rather than upstream's separate floating restore dialog; presentation and fullscreen interactions remain explicitly pending.
- Validation: initial compilation identified the missing HiDpi API feature; enabled that feature and rebuilt successfully. Compiled again after the fullscreen correction; formatting and Clippy with warnings denied passed. No unit tests added or run.


### 2026-10-14 — Distraction Free Mode (partial)

- Followed upstream `distractionFreeToggle` and default padding calculation: enter only from normal view, combine fullscreen/Post-It, hide the existing results panel and apply symmetric quarter-window-width margins. Recalculate padding during layout.
- Exit restores prior normal margins and results visibility, unwinds window modes and clears menu state. The Restore button and F11/F12 exit the combined mode. Avoid serializing temporary wide margins as normal editing preferences.
- Configurable divisor (upstream 3–9), future docking/two-view integration and interactive restoration checks remain pending.
- Validation: formatting, `target/column-build` compilation and Clippy with warnings denied passed. No unit tests added or run.


### 2026-10-14 — normal visibility preferences in special views

- Referenced upstream `Notepad_plus::postItToggle` and its `_beforeSpecialView` visibility snapshot. The conversion now reads saved toolbar/tab/status visibility when serializing preferences during Post-It or Distraction Free.
- Applying preferences while either mode is active updates the saved restoration values and keeps those controls hidden; normal gutter settings remain independent.
- Validation: `cargo fmt --all`, `cargo build -p npp-windows --target-dir target/column-build` and Clippy with warnings denied passed. No unit tests added or run. Interactive mode/preferences/relaunch verification remains unchecked.


### 2026-10-14 — revision-aware tab selection restoration

- Reviewed upstream `ScintillaEditView::saveCurrentPos` / `restoreCurrentPosPreStep` and the conversion's native selection cache. Cached ranges are now restored only when both the model selection and text revision match their captured state.
- Exposed the existing core revision identity as a cheap shared token; this avoids copying/hashing document contents on tab switches. Text edits create distinct identities, and undo restores prior identities. Background edits no longer restore stale secondary carets simply because the main caret did not move.
- Full background range remapping and selection-aware undo remain pending. Build, formatting and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run.


### 2026-10-14 — Hide Lines command and reveal markers (partial)

- Referenced upstream `ScintillaEditView::hideLines` and `hidelineMarkerClicked`, using upstream marker IDs 19/18. Added View > Hide Lines, bounded to interior document lines, merging overlapping/adjacent marked regions and moving the caret to the preceding visible line.
- Boundary symbols share the existing symbol margin; clicking either endpoint reveals the section instead of toggling its bookmark. Text and modified state are unchanged.
- Remaining integration is explicitly unchecked: tab/model refresh retention, folding/edit reconciliation, upstream visual/shortcut parity and interactive checks.
- Formatting, build and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run.


### 2026-10-14 — hidden section retention

- Added per-buffer snapshots of explicit hidden-line boundaries and text revision identity. Capture before replacing the native document and restore after fold state, preserving hidden sections across tab switches and unchanged-text refreshes.
- Referenced upstream marker-based `hideMarkedLines` behavior. Native marker positions are captured after native edits; model-driven text changes invalidate stale snapshots until full range remapping is implemented. Closed-buffer entries are removed.
- Build passed; Clippy initially requested a type alias, which was added. Rebuild, formatting and Clippy with warnings denied then passed. No unit tests added or run. Folding/edit reconciliation and interactive verification remain pending.


### 2026-10-14 — hidden sections and language folding

- Followed upstream `hideMarkedLines` calls after fold operations. Fold/unfold current, level and all commands now reapply explicit hidden sections, as does mouse release after automatic margin folding.
- Revealing an explicit section preserves closed language fold children. Hidden-section enumeration uses native marker-next queries rather than querying every document line on mouse release.
- Formatting, build and Clippy with warnings denied passed after both integration and enumeration changes. No unit tests added or run. Interactive nested-fold verification and model-edit/undo remapping remain pending.


### 2026-10-14 — Hide Lines shortcut and upstream artwork

- Added upstream Alt+H binding from `Parameters.cpp` and its menu hint. Imported the exact 14/18-pixel RGBA boundary artwork from bundled `rgba_icons.h`, with upstream 150%-DPI selection threshold and green hidden-line element color.
- Build, formatting and Clippy with warnings denied passed. No unit tests added or run. Per-monitor DPI refresh, margin sizing, hidden-margin behavior and visual verification remain pending.


### 2026-10-14 — symbol-margin DPI sizing

- Factored symbol artwork/width setup into one path used at editor initialization, preference application and `WM_DPICHANGED_AFTERPARENT`. Preserve a hidden margin while updating artwork; scale the visible width and ensure it fits the chosen marker plus padding.
- Uses the bundled ScintillaEditView 14/18-pixel artwork threshold. This fixes the previous fixed 16-pixel margin clipping larger artwork; overall application DPI awareness and per-monitor window/dialog layout remain separate pending requirements (the current manifest does not declare per-monitor DPI awareness).
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Actual monitor-transition/visual verification remains pending.


### 2026-10-14 — hidden sections after native text/fold changes

- Extended the upstream `hideMarkedLines` integration to native insert/delete and fold-level-change notifications. Queue one coalesced application event so hiding runs after the native notification stack unwinds, using marker positions adjusted by Scintilla.
- Model-driven edits/undo and deleted/orphaned section boundaries remain incomplete and explicitly unchecked; this change does not claim full hidden-line parity.
- Formatting, build and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run. Interactive edit/folding validation remains pending.


### 2026-10-14 — invalid hidden-section boundary cleanup

- Reviewed upstream `restoreHiddenLines` pairing and same-line-boundary handling. Added validation of the conversion's marker pairs after native changes: orphaned, collapsed or duplicate boundary markers trigger a rebuild from surviving complete sections.
- Rebuild shows text that would otherwise have no reveal control, reapplies closed language folds, and restores valid explicit sections. Valid adjacent sections sharing a boundary keep both markers.
- This is defensive recovery for the conversion, not proof of identical upstream deletion/undo behavior. Model-driven range remapping and interactive deletion/undo parity remain open.
- Formatting, build and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run.


### 2026-10-14 — hidden-boundary click parity

- Checked `ScintillaEditView::hidelineMarkerClicked`: shared close/open boundaries already prioritize the lower section in the conversion, matching upstream.
- Corrected unmatched hidden-boundary clicks (including an orphaned closing marker on line zero) to remove invalid markers and consume the click instead of toggling an unrelated bookmark. Native-edit cleanup remains responsible for visibility reconciliation.
- Formatting, build and Clippy with warnings denied passed after the change and line-zero correction. No unit tests added or run. Interactive boundary-deletion/undo verification remains pending.


### 2026-10-14 — Backspace unindent and Indentation preferences page

- Added persisted global Backspace-unindent, disabled by default as in upstream `NppGUI`. Applies `SCI_SETBACKSPACEUNINDENTS`, following `ScintillaEditView::setTabSettings`, and reads native state when saving preferences.
- Grouped tab width, tabs/spaces, auto-indent and Backspace-unindent on a dedicated Indentation page, avoiding overflow of the Editing page. Per-language overrides/inheritance and the three auto-indent modes remain pending.
- Compiled after the feature and again after preferences layout integration; formatting and Clippy with warnings denied passed. No unit tests added or run. Interactive indentation and preference persistence checks remain pending.


### 2026-10-14 — Basic versus Advanced indentation selection (partial)

- Added a persisted Advanced indentation choice alongside Enable auto-indent: disabled means None, enabled without Advanced uses the core basic whitespace indentation path, and Advanced retains language-aware newline formatting. Existing settings default to Advanced as upstream.
- Referenced `Notepad_plus::autoIndent` Basic handling and default `NppGUI::_maintainIndent`. The current Basic path still needs upstream previous-nonempty-line fallback; multi-caret indentation, per-language modes and exact Advanced language parity remain pending. The two-checkbox UI also differs from upstream's three mode choices.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive mode/persistence verification remains pending.


### 2026-10-14 — Basic indentation across empty lines

- Followed `Notepad_plus::autoIndent` Basic branch and `ScintillaEditView::getLineLength` (content length excludes EOL). When the line split leaves an empty preceding line, the core now searches earlier zero-length lines for a nonempty indentation source.
- Whitespace-only lines remain valid sources; an unindented nonempty line stops the search. Uses the core line-content boundaries for LF/CRLF/CR data and retains existing tabs/spaces conversion and single-edit undo handling.
- Formatting, build and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run. Interactive whitespace/selection/undo verification, three-choice UI and multi-caret indentation remain pending.


### 2026-10-14 — three-choice auto-indent UI

- Replaced the two independent indentation checkboxes with grouped None / Basic / Advanced radio controls, matching the choices and exclusivity in upstream `preferenceDlg.cpp`. Removed the old checkbox controls from creation while retaining their numeric slots for stable IDs elsewhere.
- Existing saved enabled/advanced booleans initialize the corresponding choice; OK writes the selected mode and Cancel retains the prior preferences. This completes the choice UI, not per-language/multiple-caret indentation parity.
- Formatting, build and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run. Interactive keyboard/focus/persistence verification remains pending.


### 2026-10-14 — default EOL for new documents

- Added a New Document preferences page with Windows CRLF, Unix LF and Macintosh CR defaults, persisted independently of existing documents. Referenced upstream `NewDocDefaultSettings::_format` and New Document preferences.
- New commands, replacement empty buffers after closing documents, and the initial startup buffer use the configured ending. A core constructor sets the initial ending without creating undo history or dirtying the new document; existing/opened files retain their own EOL.
- Formatting, build and Clippy with warnings denied passed using `target/column-build`. No unit tests added or run. Interactive startup/new/close-last-tab and persistence verification remains pending.


### 2026-10-14 — default encoding for new documents

- Added persisted New Document encoding selection for all nine currently supported encodings, using explicit code-page labels rather than treating Windows-1252 as system ANSI. Referenced upstream `NewDocDefaultSettings` Unicode mode/code-page defaults.
- Startup, File > New and replacement empty buffers initialize both encoding and EOL without creating edit history or marking the document modified. Existing/opened buffers remain unchanged. Default remains UTF-8 without BOM.
- System ANSI, remaining legacy encodings and ANSI-as-UTF8 policy remain unchecked. Formatting, build and Clippy with warnings denied passed using `target/column-build`; no unit tests added or run. Interactive persistence/save-byte verification remains pending.


### 2026-10-14 — default built-in language for new documents

- Added a persisted New Document language selector using all built-in language registry entries, corresponding to upstream `NewDocDefaultSettings::_lang`. New/startup/replacement empty tabs receive the chosen explicit language before refresh, enabling its highlighting, folding and existing editing helpers.
- Existing/opened files retain their own language selection/detection. Invalid saved language IDs fall back to Normal Text. Runtime UDL defaults remain outside this built-in selector.
- Initial build identified iteration over the lazy registry; corrected to iterate the resolved collection. Formatting, rebuild and Clippy with warnings denied passed. No unit tests added or run. Interactive startup/Save As/persistence behavior remains pending.


### 2026-10-14 — default language versus explicit selection on Save As

- Referenced upstream `Buffer::setFileName` and `_hasLangBeenSetFromMenu`. Track new-document language defaults separately from explicit language choices: successful path-changing saves release the default and detect the filename's language; explicit menu/UDL/session selections remain selected.
- Failed saves and Save Copy retain the original language. Closed-buffer default flags are discarded. This corrects the initial default-language implementation, which otherwise prevented extension detection even for Normal Text defaults.
- Formatting, build and Clippy with warnings denied passed after the integration and cleanup. No unit tests added or run. Interactive Save As/default/explicit-selection verification remains pending.


### 2026-10-14 — optional recent-file existence checks

- Added persisted Check recent files exist at startup on Recent Files preferences, disabled by default like upstream `_checkHistoryFiles`. After loading history, enabled checking removes entries that are missing, inaccessible or not files, and persists the filtered history. Disabled checking retains entries for later retry.
- Referenced upstream `Notepad_plus.cpp` recent-file initialization and `CheckHistoryFiles` configuration. Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive/network-path startup behavior remains pending.
- Reviewed extra-new-document-at-startup behavior: upstream gates it on automatic session restoration. That feature remains pending with session restoration rather than introducing different command-line-only behavior.


### 2026-10-14 — recent-history placement

- Added persisted Put recent files in a submenu preference, referencing upstream `putRecentFileInSubMenu`. Both layouts include ordered history entries, Open All and Clear History with empty-history disabled states, before Restore Last Closed File.
- Rebuild removes only history-owned items and transfers native labels/state/IDs into File for direct placement. Retains the conversion's existing submenu default for old settings; upstream's fresh default is direct placement.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive placement switching, accelerators and long-path labels remain pending.


### 2026-10-14 — dynamic menu command isolation

- Fixed a confirmed collision: recent-file IDs 62000–62029 overlapped UDL IDs 62000–62499, and Post-It Restore ID 62031 overlapped the 32nd UDL. The language dispatcher runs before recent-file dispatch, so installed UDLs could intercept history commands.
- Moved history/control IDs to 63000–63029/63031, centralized the existing UDL range and added a compile-time non-overlap assertion. Existing serialized UDL IDs remain unchanged. Checked against the compatibility layer's separate plugin range (22000–22999) and upstream menu command definitions.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive history/UDL/Post-It coexistence verification remains pending.


### 2026-10-14 — avoid rebuilding unchanged recent menus

- Referenced upstream `LastRecentFileList::updateMenu` calls on history changes. Added a snapshot of File-menu identity, ordered history paths, label mode and submenu placement; unchanged status/caret updates now return without deleting/recreating menu items or allocating label buffers.
- History additions/removals/order changes, label preferences, placement changes and a different File menu handle trigger rebuilding. Snapshot path comparison does not access disk.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive menu/popup stability and broader large-file performance remain pending.


### 2026-10-14 — selection foreground override preference

- Added persisted Apply selection foreground color to selected text on Editing preferences, disabled by default like upstream `_selectedTextForegroundSingleColor`.
- Followed upstream selection element setup: apply/remove the override for active, inactive, additional and secondary selections. Disabled preserves lexical foreground colors; enabled uses the conversion's existing white selection foreground. Configurable selection colors still depend on pending Style Configurator/theme support.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive contrast, multi-selection and inactive-selection checks remain pending.


### 2026-10-14 — advanced-scrolling preference (partial)

- Added persisted Disable advanced scrolling on Mouse preferences. Following upstream ScintillaEditView wheel handling, enabled advanced scrolling redirects editor wheel events to the hovered control; disabling it retains delivery to the editor. Synaptics cursor windows retain editor delivery.
- Redirection is restricted to the application UI thread and guarded against recursive forwarding. Cross-application redirection and right-button-wheel behavior remain parity gaps; normal Shift-wheel/horizontal-wheel/zoom paths remain in place.
- Formatting, build and Clippy with warnings denied passed after implementation and reentrancy guard. No unit tests added or run. Physical mouse/touchpad and hovered-panel verification remain pending.


### 2026-10-14 — right-button wheel tab switching (partial)

- Referenced `ScintillaEditView` wheel forwarding and `NppBigSwitch.cpp` right-button-wheel previous/next document dispatch. Editor right-button wheel now switches to the previous tab for positive delta and next for negative delta, before Shift-wheel/zoom handling.
- Consume the gesture's right-button release and mouse context-menu request to avoid a trailing popup; ordinary right clicks reset the gesture state. The MRU task-list popup/order preference remains unimplemented, so this uses existing tab-order navigation.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Physical gesture, focus/capture and context-menu verification remain pending.


### 2026-10-14 — mouse Back/Forward document navigation

- Referenced upstream `NppBigSwitch.cpp::WM_APPCOMMAND`. Browser Back/Forward application commands now dispatch previous/next tab navigation, masking input-device bits as the Windows macro does. Other application commands continue through default processing.
- Uses existing tab navigation/selection preservation; two-view and MRU behavior remain pending with their respective features.
- Initial build caught missing constant imports; corrected from windows-sys SystemServices. Formatting, rebuild and Clippy with warnings denied passed. No unit tests added or run. Physical extra-button/keyboard application-command verification remains pending.


### 2026-10-14 — SQL backslash escape preference

- Added a Language preferences page with persisted SQL backslash escaping, enabled by default as upstream. Applies the exact `sql.backslash.escapes` lexer property from `ScintillaEditView::setSqlLexer` and `NPPM_INTERNAL_SQLBACKSLASHESCAPE` handling.
- Changing the setting recolors active SQL immediately; switching into SQL applies it when constructing the lexer. Other language properties are untouched.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive escaped-quote/tab-switch/persistence checks remain pending.


### 2026-10-14 — compact or flat Language menu

- Added persisted compact Language menu preference, enabled by default as upstream `_isLangMenuCompact`. Rebuilds immediately between first-letter submenus and an alphabetical flat list, retaining Normal Text and the UDL management/selection entries.
- Applies the current language check state through the existing preference refresh/caption path. Language exclusions and full UDL support remain pending.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive menu sizing, selection checks and UDL coexistence verification remain pending.


### 2026-10-14 — built-in language menu exclusions

- Added a persisted multi-select exclusion list on Language preferences. Applying changes rebuilds compact or flat menus and omits empty alphabetical groups; Normal Text remains available. Hidden languages still participate in file detection, syntax highlighting and new-document defaults.
- Referenced upstream preference remove/restore handlers and their empty-submenu cleanup. Current UI uses one exclusion list instead of upstream's two available/disabled lists; exact presentation remains pending, as does UDL menu management.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive exclusion/reinclusion, keyboard selection and persistence checks remain pending.


### 2026-10-14 — Available/Disabled language lists

- Replaced the temporary exclusion-selection UI with alphabetical Available items and Disabled items lists plus remove/restore buttons, following upstream preference resources and handlers. Supports moving multiple selected entries and keeps stable command IDs as native item data.
- OK persists the Disabled list; Cancel leaves stored settings unchanged. Lists fit beneath SQL/compact options and retain edits when switching preference pages.
- Formatting, build and Clippy with warnings denied passed. No unit tests added or run. Interactive keyboard/focus, list movement and menu refresh verification remain pending.


### 2026-10-14 — Date & Time preferences

- [x] Custom date/time format editor with live preview and persisted reverse default order on a dedicated preferences page. Uses the existing Windows formatter based on upstream `Common.cpp::getDateTimeStrFrom`, default `yyyy-MM-dd HH:mm:ss`, and the insertion order from `NppCommands.cpp`.
- Applying preferences updates insertion behavior and the existing date-first menu check. Changing that menu option now persists it too; Cancel retains prior settings.
- Formatting and application build passed. No unit tests added or run. Interactive preview, Cancel/OK and restart verification remain pending.
- [ ] **Verify:** Date & Time preferences layout, locale-specific format preview, insertion, Cancel/OK, menu synchronization and restart persistence.


### 2026-10-14 — rendering technology preference (partial)

- [x] Persisted rendering-mode selector with GDI and the four DirectWrite variants from upstream `preferenceDlg.cpp`. Applies `SCI_SETTECHNOLOGY` during startup, matching upstream's restart-required preference; changing other settings does not recreate the rendering target.
- Retained the conversion's existing GDI default while renderer compatibility remains unverified. Upstream defaults to DirectWrite and detects Wine/Windows Server Core before applying the engine.
- [ ] **Partial:** complete rendering-mode parity: Wine/Server Core detection, upstream DirectWrite default, unavailable-engine reporting/fallback and RTL compatibility. Verify all available modes, font smoothing, zoom, folding/margins and restart persistence on actual hardware.
- Formatting, application build and Clippy with warnings denied passed. No unit tests added or run; no interactive rendering verification claimed.


### 2026-10-14 — renderer environment guards

- [x] Detect Wine through `ntdll!wine_get_version` and Windows Server Core through the 64-bit `InstallationType` registry value, following upstream `ScintillaEditView::init` and `Common.cpp::isCoreWindows`. Force GDI in those environments and disable the rendering selector with an explanatory label; retain the requested preference for other environments.
- [ ] **Partial:** upstream DirectWrite default and RTL/hardware verification remain open. Legacy Server Core detection and renderer-load fallback reporting are implemented below.
- Formatting, application build and Clippy with warnings denied passed. No unit tests added or run. Wine/Server Core execution has not been verified on this workstation.


### 2026-10-14 — legacy renderer guards and load fallback

- [x] Legacy Server Core detection using `GetVersionExW` and `GetProductInfo`, matching all 15 product SKUs in upstream `Common.cpp::isCoreWindows`, before the modern registry fallback.
- [x] Check `SCI_GETTECHNOLOGY` after requesting a renderer. If Scintilla rejects the requested engine, explicitly restore GDI and report the fallback with the Preferences location. The persisted requested mode remains available for a later launch.
- Referenced bundled `ScintillaWin.cxx::SetTechnology`: failure to load Direct2D/DirectWrite leaves technology unchanged. This detection does not establish successful later GPU target creation; device-loss/runtime graphics verification remains pending.
- Compiled after each change; formatting and Clippy with warnings denied passed. No unit tests added or run. Legacy OS and graphics-load failure paths have not been exercised interactively.


### 2026-10-14 — Find in Files disk-content preference

- Added a Searching preferences page with Prefer on-disk file content over open buffers, disabled by default as upstream. Referenced `Notepad_plus.cpp` handling of `_fif_ignoreunsavedChangesInOpenedFiles` and the upstream preference resource.
- Enabled searches omit open-buffer snapshots so the existing worker reads disk contents with its normal decoding/error handling. Disabled searches retain open-buffer snapshots; neither choice saves or discards unsaved edits.
- Formatting, application build and Clippy with warnings denied passed. No unit tests added or run. Interactive dirty-buffer versus disk comparison, result navigation and persistence remain pending.
- Work stopped at the user's request after this feature. Find All in Current Document and the other unchecked parity items remain unfinished.
