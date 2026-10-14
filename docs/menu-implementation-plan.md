# Non-plugin menu implementation plan

The supplied `PowerEditor/src/Notepad_plus.rc` and `menuCmdID.h` are the
reference. Completion requires working commands, not merely menu entries.
No new unit tests are planned for this pass; use build checks and interactive
smoke checks for changed workflows.

1. Repair tab interactions (close, middle click, context actions, drag ordering,
   selection/scroll preservation) and Find in Files sizing, focus and dismissal.
2. Complete File: printing, rename/delete, sessions, recent files, workspace
   folders and recovery. Retain the existing safe-save and close confirmation paths.
3. Complete Edit/Search: comments, column/multiple selection, autocomplete,
   regex in the current document, unified search tabs, marking and replace in files.
4. Replace the plain EDIT renderer with an editor supporting syntax styling,
   gutters, folding, indicators, wrapping and multiple views. This is necessary
   for much of View and Language; exposing switches without rendering is insufficient.
5. Implement Language and Settings: lexer selection, user languages, preferences,
   styles, shortcuts, import/export and persistence.
6. Implement Macro and Run: record text and commands, playback, named persistence,
   repetition, command variables and saved commands.
7. Complete View/Window/panels and Help, then audit every upstream menu command
   and record remaining behavioral discrepancies explicitly.
8. Keep document operations portable; macOS/Linux frontends remain required
   separately from Windows menu completion.

The existing parity document contains historical evidence and stale pending
entries. Neither it nor generated command constants establishes full parity.

## Implemented in this pass

- Tab close targets, middle-click close, drag ordering, context commands,
  numbered-tab shortcuts and move-to-start/end. Native tab updates retain
  the control and use the existing unsaved-document confirmation flow.
- Themed common controls, scalable Consolas, toolbar with tooltips and undo
  availability, logical line numbers, word wrap, full screen and always on top.
- Find/Replace/Find in Files navigation with query transfer, consistent client
  sizes, explicit search-mode controls, wrap/count controls, separate file-search
  Stop/Close and busy button states. Regex handling resides in the portable core.
- Comment/uncomment, word completion, Delete, selection redaction and selected
  path opening. Comments choose common delimiters from the filename extension.
- Rename without overwriting another destination; Recycle Bin deletion retains
  the buffer. Native printer dialog, paginated text printing and selection scope.
- MD5/SHA-1/SHA-256/SHA-512 generation from text, a file or selected text.
- Ten Window sorting commands; basic command/text macros with repetition and
  JSON save/load; Run with file and selected-text variables; Help actions.
- Preferences dialog and JSON persistence for its supported options.

## Validation and remaining limitations

### Search and menu follow-up

- File search now detects BOM-less UTF-16 LE/BE, excludes binary/control-heavy
  content, preserves the detected encoding in each hit, and shows canonical
  Windows paths without their internal extended-path prefix. Result rows use
  Consolas, shaded file/search headings, aligned line numbers, expanded tabs,
  escaped controls, and one row per matching line (hit counts still count matches).
- Genuine embedded NUL files can open, edit, reload and save. The native view
  renders NUL directly through Scintilla. Clipboard transfers use a length-delimited private UTF-8 format
  alongside readable standard text so copying does not truncate at NUL.
- Language includes all 94 built-in choices from the supplied resource, grouped
  alphabetically, extension detection, per-buffer overrides, session persistence,
  status/check marks, comment delimiters, and keyword completion. Custom language
  definitions support names, extensions, comments and keywords, persisted under
  `%APPDATA%/NotepadPlusPlusRust/userDefineLangs/languages.json`. This is a basic
  definition wizard and JSON format, not the upstream UDL XML/styling editor.
- Added Paste HTML/RTF Content, Function/Keyword Completion, EOL to Space, and
  Trim both and EOL to Space. Unavailable comment operations are disabled for
  languages without the corresponding comment delimiters.
- A temporary smoke executable checked UTF-16 detection, search offsets,
  binary exclusion, NUL/control-picture editing and lossless encoding. Native UI
  checks found 9 matches across UTF-8/UTF-16 LE/BE fixtures, opened a UTF-16 hit,
  selected Python, exercised comment/undo, and opened/copied/pasted/edited/saved
  a NUL fixture. The saved bytes were verified against the expected bytes.
- The normal release executable was in use. The follow-up build is at
  `target/review-build/release/npp-windows.exe`.

### Syntax, folding and formatting follow-up

- Replaced EDIT with the bundled Scintilla/Lexilla, statically linked. Generated
  94 built-in language profiles from upstream lexer/style/keyword definitions
  (`python scripts/generate-syntax.py`, followed by `cargo fmt --all`).
- Added native fold markers, fold/unfold all/current/levels 1–8, per-tab fold
  retention, syntax colours and indentation guides.
- Added Format Document/Selection, JSON/XML layout, syntax-aware code reindent,
  and language-aware Enter indentation. Formatting retains strings/comments,
  JSON token spelling/order and XML mixed content. Generic reindent is not a
  canonical language formatter. Custom UDL styling remains outstanding.
- Interactive checks verified Rust/Python/JSON/XML colouring, margin folding,
  fold/unfold-all, collapsed-state retention across tabs, Rust reindent and its
  single-step undo, smart Enter insertion/undo, and JSON/XML formatting and saved
  output. XML mixed content remained intact. Workspace Clippy passed with warnings
  denied; no unit tests were added or run for this follow-up.

Compilation and Clippy cover all existing workspace targets. No unit tests were
added or run. An existing empty-vector assertion needed an explicit type to
compile against the existing regex dependencies.

Native UI checks covered document input, comment/undo, new tab, middle-click
close, Find in Files opening/dismissal, query transfer to Find, Replace dialog
layout, and zero-width regex Replace All with undo. Printer output, external
command launch, file deletion and all advanced macro combinations were not
exercised interactively.

The inventory script includes alternative resource definitions and reports
references, not semantic implementation. Dynamic tab command ranges and
rewrite-specific aliases need manual classification. Many commands still lack
handlers; do not mark this plan complete.

Multiple selections and split views are not integrated with the Rust document
model. Search tabs currently switch owned dialogs rather than one shared
dialog object. Replace in Files, Mark indicators, advanced search result panels,
full macro navigation/shortcut management, saved Run commands, advanced language
settings and project/function panels remain. Macros use the rewrite's JSON format,
not upstream shortcuts.xml. Printing is plain text without upstream page setup
or style options. Preferences saves supported options when its OK button is used.
macOS/Linux UI implementation is unchanged.
