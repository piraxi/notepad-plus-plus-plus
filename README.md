# Notepad++ Rust rewrite

A Windows-first Rust rewrite of the supplied Notepad++ source, with shared editor
code intended for Windows, macOS, and Linux. The goal is full feature parity,
including plugins. **This is an early implementation, not a usable replacement
for Notepad++ yet.** A basic Windows graphical executable is available; application
plugin hosting and most advanced editor features remain unimplemented.

## Layout

- `notepad_plus_plus/`: supplied upstream reference source, including Scintilla
  and Lexilla. Existing source and license notices are retained.
- `crates/editor-core/`: platform-independent document state, encodings,
  selections, edit history, literal search/replace, and buffer management.
- `crates/npp-compat/`: command IDs generated from the upstream header, Windows
  plugin ABI declarations, and a DLL loader tested with compiled fixtures. This
  is not yet a functioning application plugin host.
- `Windows/`: native Windows application, dialogs, text-view bridge, and file writer.
- `MacOS/` and `Linux/`: future platform implementation destinations.
  Each platform directory contains an empty `.gitkeep` file.

## Build and test

Install Rust 1.85 or later. Windows builds require the MSVC linker and Windows SDK
from Visual Studio Build Tools. From the repository root:

```text
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The Windows adapter uses Microsoft's `windows-sys` API bindings; the portable
core uses `encoding_rs` for portable legacy encoding tables and BLAKE3's pure-Rust
mode for disk-content fingerprints. `Cargo.lock` is
committed for reproducible application development. Build output goes in
`target/` and is excluded from Git along with native artifacts for each OS.

Run the Windows editor, optionally opening files from the command line:

```text
cargo run -p npp-windows -- README.md
cargo build --release -p npp-windows
```

The release executable is `target/release/npp-windows.exe`. The UI currently
provides **Search > Find in Files** (`Ctrl+Shift+F`) with folder browsing,
include/exclude filters, recursive and hidden-folder options, Normal/Extended
and regular-expression search. Results appear below the editor; double-click or
press Enter to open a match, or use F4/Shift+F4 to navigate. Right-click results
to copy, clear, stop searching or hide the panel. Search inputs keep a recent
history for the current run. Example filters: `*.rs *.toml !*.bak !+\target`.
Regex syntax is not yet a complete match for Notepad++'s Boost engine.

The UI currently
accepts multiple files dragged from Explorer, including Unicode paths. Dropping
an already-open file activates its existing tab. Folder-drop workspace behavior
and automatic legacy-encoding detection remain pending.
The UI also
provides document tabs, New/Open/Save/Save As/Close, unsaved-change prompts,
undo/redo, clipboard editing, Select All, encoding and line-ending conversion,
and a position/encoding status bar. Standard Ctrl+N/O/S/W/Z/Y/A shortcuts and
Ctrl+Shift+S are supported; clipboard shortcuts are provided by the text control.
File also provides Save All, Reload from Disk, Close All, Close All but Current,
Close All to the Left/Right, and Close All Unchanged. Batch operations stop on
cancellation or save failure. Reload asks before discarding edits, validates the
replacement before updating the buffer, preserves line/character position where
possible, and clears the old undo history after success.
Restore Last Closed File (Ctrl+Shift+T) reopens the latest closed saved file from
disk, without restoring discarded edits. The initial recent-file list holds ten
paths and their saved encodings in memory; persistence and configurable history
limits remain pending. Discarded encoding conversions do not affect reopening.
The View menu provides next/previous tab (Ctrl+Page Down/Up) and move tab
left/right. Navigation wraps; tab movement stops at either end.
Untitled names stay with their buffers when tabs move or close. New documents
reuse the lowest available `new N` number after a buffer is saved or closed;
tabs, window titles, and plugin filename queries use the same workspace name.
Edit also provides Duplicate Line (Ctrl+D), upper/lowercase conversion
(Ctrl+Shift+U/Ctrl+U), Invert Case, and leading/trailing/both whitespace trimming. These
commands use the shared core's single undo transaction. Case conversion runs
through the Windows adapter. Uppercase, Lowercase, Invert Case and Random Case use the source's Windows
UTF-16 character mapping, preserving one code unit per mapped character.
TAB to Space and Space to TAB (Leading/All) use the selected tab width, count Unicode
characters, and expand partial selections to complete lines. Changes preserve
line endings and form a single undo transaction. View offers session-wide tab
widths of 2, 4 (default), or 8 columns, shared by native rendering, conversion,
status and plugin column queries. Persisted and language-specific settings remain pending.
Tab and Shift+Tab now use shared-core indentation and undo. Tab inserts within
text or advances leading indentation; Shift+Tab reduces indentation or moves to
the previous tab stop within text. Multiline selections indent complete lines,
exclude an unselected final line, and skip empty lines when indenting. Edit also
offers Increase/Decrease Line Indent, preserving the supplied source's distinct
single-line behavior. View's Replace Tab with Spaces preference generates spaces
for subsequent Tab and indentation commands without converting existing text.
The preference lasts for the current session; advanced selections remain pending.
Delete Line (Ctrl+Shift+L) removes the caret line, including its ending when
present. Transpose Line (Ctrl+T) swaps its contents with the previous line while
preserving line-ending positions. Both use shared-core undo and respect read-only files.
Delete Word Left/Right (Ctrl+Backspace/Delete) uses the Rust core's Scintilla
word-class boundaries, with undo and read-only protection.
Ctrl+Left/Right uses those boundaries for navigation; adding Shift retains the
selection anchor, including when extending backwards in read-only text.
Home toggles between the first non-space/tab character and the line start;
Shift+Home extends the selection to that position.
End moves to the physical line's content end. Ctrl+Home/End moves to the document
boundaries; adding Shift to these shortcuts preserves the selection anchor.
Ctrl+[ and Ctrl+] move between paragraphs separated by empty or space/tab-only
lines. Adding Shift extends the selection.
Ctrl+/ and Ctrl+\ navigate word parts such as `someValue` and `HTTPServer2_name`;
Shift extends the selection. These use the source's ASCII case/digit rules.
Delete to Start/End of Line (Ctrl+Shift+Backspace/Delete) deletes from the caret
to the respective boundary, preserving adjacent lines and using shared-core undo.
Remove Empty Lines and its Including Blank Characters variant operate on the
exact selection, or the whole document without a selection. The latter also
removes lines containing only ASCII spaces/tabs. Both remove an eligible trailing
empty line and form one undo transaction.
Join Lines (Ctrl+J) joins selected complete lines, preserves indentation, and
inserts a separating space unless one already precedes the removed ending.
The last selected line's ending is retained; a single selected line is unchanged.
Insert Blank Line Above/Below (Ctrl+Alt+Enter / Ctrl+Alt+Shift+Enter) uses the
configured line ending and places the caret on the inserted line. In mixed-ending
text, the supplied offset calculation is retained; offsets that would split UTF-8
are rejected without changing the document.
Enter now runs through the shared core. View offers optional Basic Auto-Indent,
which copies the preceding line's indentation using the current tabs/spaces
preference. Newline and indentation form one undo transaction. The option is
session-only and initially off; language-aware automatic indentation remains pending.
Move Lines Up/Down (Ctrl+Shift+Up/Down) moves the caret line or selected block,
keeps the moved block selected, and supports undo. Moving across the last line
handles both empty and unterminated document tails.
Zoom In/Out and Restore Default Zoom use Ctrl+numeric-keypad Plus/Minus/Divide.
Zoom changes the native editor font without editing the buffer, and preserves
tab alignment. Zoomed text uses a scalable font; restore returns to the original
stock font. Zoom is session-wide; font preferences and per-view persistence remain pending.
Ctrl+mouse wheel also zooms, accumulating partial wheel movements and respecting
the Windows wheel-disabled setting. Ordinary wheel scrolling remains native.
Shift+wheel and horizontal-wheel input scroll sideways using the Windows
characters-per-notch setting, including accumulated partial movements.
Go To (Ctrl+G) opens a modeless dialog for line numbers or UTF-8 byte offsets,
showing the current position and maximum. Navigation works in read-only buffers,
clamps out-of-range values, and avoids splitting Unicode characters or CRLF pairs.
Cut Lines (Ctrl+L) and Copy Lines (Ctrl+Shift+X) operate on all lines touched by
the selection, preserving their line endings in Unicode clipboard text. Cut
leaves the buffer intact if the clipboard cannot accept the text.
Copy All File Names and Copy All File Paths copy the open tabs in their displayed
order, including untitled names, with CRLF-separated entries.
Copy Current Full File Path, File Name and Directory Path copy the active tab's
identity without changing its text or selection, including in read-only documents.
File offers Open Containing Folder in Explorer and Open in Default Viewer for
saved files. These act on the on-disk file and do not save pending edits.
Begin/End Select (Ctrl+Shift+B) remembers a view position, tracks intervening edits,
and selects from that position to the caret on its next invocation. The stream
selection command works in read-only documents; rectangular selection is pending.
View → Summary displays document and selection counts, plus the path and local
creation/modification times when the saved file is accessible. Counts follow the
source's encoding-specific rules and punctuation-based word definition.
The status bar shows selected character/line counts, or the caret's one-based byte
position without a selection, alongside the document's byte and line totals.
View offers Show Status Bar and Show Tab Bar. Hiding either expands the editor;
these settings currently last for the running session. Tab navigation remains
available through keyboard shortcuts while the tab bar is hidden.
Show Menu Bar (Ctrl+Alt+M) hides or restores the menu. The shortcut remains active
while hidden, and plugin menu queries and updates retain access to the menu.
Save a Copy As writes the current encoding and line endings to another file while
retaining the active document's name, dirty state, read-only flags and undo history.
Line sorting includes case-insensitive and length-based ascending/descending
orders. These follow the source's default C-locale case comparison and UTF-16
length measurement, using the configured line-ending delimiter and supporting undo.
Integer sorting compares numeric chunks within text, including negative values,
leading zeros and numbers larger than machine integers.
Decimal-dot and decimal-comma sorting support both directions, stable equal values,
and numeric prefixes. Invalid or out-of-range values report their line number
without partially sorting the document.
Locale sorting uses Windows regional collation with case-insensitive numeric
ordering. It moves existing line endings with their text, excludes a trailing
empty line, preserves selection direction, and keeps the caret on its original
line. Locale option controls and rectangular sorting remain pending.
Random Line Order shuffles ordinary sort blocks, including a final empty entry,
with undo support. Its engine and seeded permutations match the tested MSVC v143
implementation used by the supplied Windows project.
Short and long date/time insertion use Windows regional formats without seconds,
replace the selection in one undo step, and support a session-only Date Before
Time option. Custom insertion currently uses the source default
`yyyy-MM-dd HH:mm:ss`; editing and persisting that preference remain pending.
Proper Case and Sentence Case each offer Force
(lowercase remaining letters) and Blend (retain existing capitals), using the
source's Windows character rules and one-step undo for a stream selection.
Editing commands are disabled for read-only buffers and ignore keyboard dispatch
without opening error dialogs. Copy and navigation remain available; Undo/Redo
menu availability follows the active buffer's history.
The Replace dialog also tracks read-only state across buffer switches, keeping
Find available while disabling Replace/Replace All. Queued replacement requests
are checked again against the active buffer before editing.
Edit offers Set Read-Only and Set/Clear Read-Only for All Documents. User editing
protection is separate from file permissions, so clearing it cannot unlock a
file marked read-only by the filesystem. These commands do not alter text/history
or disk permissions; the user flag survives reload and saving.
Set File Read-Only Attribute separately toggles the Windows file attribute for
saved files, preserving other attributes and user protection. Access-denied
failures are reported; administrator elevation is not yet implemented.
On app activation, background metadata checks refresh read-only attributes for
open files without reloading text or disturbing selection/history. Inaccessible
files retain their previous protection state. Deleted files are marked as needing
a save so closing cannot silently discard the retained copy; file read-only
protection is cleared while user protection remains. Background byte
fingerprints also detect external changes and deletion, shown in the status bar
without replacing the buffer. Reload remains an explicit action; continuous
watching and upstream-style notification dialogs remain pending.
Ascending/descending lexicographic sort and reverse line order act on complete
selected lines, or the whole document without a selection. Lexicographic
comparison uses UTF-16 ordering to match the Windows source. Whole-document
sorting includes the final empty item, if present. As in the supplied source,
sorting splits and rejoins on the configured line ending; other endings remain
inside those items. Locale and rectangular-column sorting remain outstanding.

Remove Consecutive Duplicate Lines and Remove Duplicate Lines operate on selected
complete lines, or the whole document without a selection, with one undo step.
Comparison is case-sensitive. As in the supplied source, consecutive removal
compares line endings too and handles an unterminated final duplicate specially;
removing all duplicates splits on the configured line ending and keeps the first
occurrence. Mixed-ending documents can therefore produce different results
between the two commands.

Find (Ctrl+F) and Replace (Ctrl+H) use modeless native dialogs. F3/Shift+F3
repeat search forwards/backwards with wrapping. Ctrl+Alt+F3 searches the selection temporarily (Shift
reverses direction), expanding the word at the caret when needed. This uses
case-insensitive literal matching with wrapping and preserves the saved search.
Ctrl+F3 (Shift for reverse) instead saves the selection as the search query and
uses the current case/whole-word options, with literal matching for that search.
Search > Wrap Around also controls Count/Replace All scope: with wrapping off,
forward operations start at the selection's beginning and backward operations end
at its end. The in-selection option overrides those bounds.
Case-insensitive literal search uses Scintilla's supplied folding mappings,
including sharp-S and ligature expansions, while retaining original match offsets.
Whole-word boundaries follow
Scintilla's default UTF-8 classes, including combining marks and punctuation
runs, using the supplied Unicode 16.0 table. Match case and whole-word
options are supported, and Replace All is a single undo transaction. The
Search menu's Count Occurrences counts non-overlapping matches throughout the
document without moving the selection. It reads the open Find/Replace dialog's
current text and options, or reuses the last query when that dialog is closed.
Enable Search > Count / Replace All in Selection to restrict those two commands
to complete matches inside the current selection. An empty selection matches
nothing. Replace All adjusts the selection to cover the changed text, and undo
restores its previous bounds; Find Next/Previous remain navigation commands.
Find Next/Previous stop at the requested match instead of collecting all matches;
Count and Replace All also scan without an intermediate list of match ranges. The
initial literal matcher compares Unicode lowercase scalars; word characters
are letters, numbers, and underscore. This does not yet reproduce Scintilla's
configurable word classes or full Unicode case folding.
Search > Extended Search Mode enables `\r`, `\n`, `\t`, `\0`, `\\`, and
fixed-width numeric escapes: `\b` (8 binary digits), `\o` (3 octal), `\d`
(3 decimal), `\x` (2 hexadecimal), and `\u` (4 hexadecimal UTF-16 digits).
The mode applies to finding and replacement, including F3 and Replace All;
dialog titles show Normal or Extended. Unknown/incomplete escapes remain literal.
Paired surrogate escapes are supported; unpaired surrogates are rejected.
The core supports NUL matches/replacements, but the native view rejects NUL
replacement text before editing. Regex, search in files, and the complete
Notepad++ search dialog remain outstanding.
The Search menu also provides Go to Matching Brace and Select Matching Braces
for `()`, `[]`, and `{}`. These prefer the brace before the caret and handle
nesting. Matching is currently plain text; lexer-aware filtering and automatic
brace highlighting remain pending.

The initial view uses the Windows multiline text control. A projection converts
UTF-16 control positions to UTF-8 core positions and preserves unchanged mixed
line endings when editing. Files containing embedded NUL are explicitly rejected
by this view. This control does not yet provide syntax coloring, line numbers,
folding, rectangular selection, or the Scintilla message interface.

## Implemented foundation

- UTF-8, UTF-8 BOM, and UTF-16 LE/BE with or without a BOM round trips; malformed input is
  rejected instead of replaced. Explicit Windows-1252, Windows-1251 and Shift-JIS
  opening/conversion is available in the File/Encoding menus. Reload retains an
  explicit legacy or BOM-less UTF-16 encoding. Conversion and saving reject characters that cannot
  round-trip, before writing any bytes. Automatic legacy detection, system ANSI,
  remaining code pages, and BOM-less UTF-16 auto-detection remain TODO. BOM-less
  UTF-16 requires the explicit File menu opening action; its leading U+FEFF, if
  present, is treated as text. Recent-file history retains explicit encodings
  for Restore Last Closed File.
- UTF-8 byte-position selections, Unicode-boundary validation, undo/redo,
  savepoints, and document read-only state.
- Mixed CRLF/LF/CR preservation, explicit ending conversion, and line indexing.
- Literal find with direction, wrap, case and whole-word options, plus
  transactional replace-all.
- Stable buffer IDs and refusal to close modified buffers without discard.
- Save callback contract that updates document state only after a successful
  write. The Windows writer flushes staged content on the same volume before
  installation. Existing files use `ReplaceFileW` with metadata merging and a
  recovery backup; new files are installed without overwriting a racing writer.
  Tests verify locked/read-only failures, named-stream and creation-time
  preservation, and simulated partial-failure recovery. Save As refuses to
  overwrite a file open in another tab.
- Upstream menu command constants, Windows plugin and notification ABI layouts,
  and a DLL loader with export validation, command metadata, callback dispatch,
  command-ID assignment and owned library lifetime. Tests compile small Rust
  DLL fixtures using `rustc` from PATH and verify unloading on success/failure.
  A registry assigns unique IDs from the upstream reserved command range,
  preserves menu separators, routes callbacks to the owning DLL, and broadcasts
  notifications. Duplicate-module and capacity rejection retain DLL ownership
  for orderly shutdown; multi-plugin fixture tests cover routing and cleanup.
  The Windows Plugins menu renders registered commands and dispatches them to
  their owning DLL. Shutdown notification precedes host-window destruction and
  DLL unloading. Native tests use a controlled fixture; the application still
  does not discover or load installed third-party plugins, and compatible
  most Notepad++/Scintilla host messages remain pending. An initial synchronous
  NPPM query subset reports buffer IDs, tab positions/counts, encoding and EOL
  format. Per-window snapshots allow these queries from inside DLL callbacks;
  closed-buffer identity tokens are retained until the window closes to avoid reuse.
  File-path queries include the length-then-copy buffer-ID API and current full
  path, directory, filename, name without extension, and extension. Lengths are
  UTF-16 code units; capacity checks include room for the terminator. The legacy
  zero-capacity convention requires the plugin to supply a sufficiently large buffer.
  Executable-path and installation-directory queries return the running host's
  location, using the source's MAX_PATH-sized module buffer.
  Caret line/column queries are zero-based and expand tabs at the selected stops;
  the status bar uses the same calculation with one-based labels. The current-line
  text query includes its ending and preserves the supplied implementation's
  empty result for lines of 2,048 UTF-8 bytes or more.

## Architecture and remaining scope

The core contains no OS handles, GUI dependencies, or unsafe Rust. Windows UI,
filesystem replacement, dialogs, and legacy DLL interaction belong in platform
adapters. macOS and Linux should reuse the same core and command behavior.

Existing Notepad++ plugins receive Windows and Scintilla handles and use native
messages. Full Windows binary-plugin compatibility requires reproducing that
message contract, notification layouts, docking, menus, and lifecycle behavior.
The ABI declarations here are only the start. Windows DLLs cannot be treated as
cross-platform plugins; portable extensions will need an additional API.

Full parity remains outstanding: the complete GUI/editor renderer, syntax lexing/folding,
multi-view editing, rectangular/multiple selections, regex and file search,
all editing commands, sessions/recovery, preferences/themes/localization,
macros, project/function panels, printing, plugin loading/administration and
compatibility, and packaging. Track implementation evidence in
[`docs/parity.md`](docs/parity.md); command presence is not proof of behavior.

Saving the current file checks its bytes against a fingerprint from the last
successful load/save. An external modification or deletion requires an explicit
overwrite choice; declining leaves both versions untouched. This is a pre-save
check, not a filesystem lock: another process can still write between the check
and replacement. Automatic change notifications/reload, hard-link identity, and
recovery after system failure remain outstanding. Windows metadata preservation uses
[`ReplaceFileW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)
without ignoring ACL/metadata merge errors. Rare partial replacement failures
retain a `.npp-rust-*.save` directory beside the destination and report its path;
the original is restored only if the destination is absent. These recovery files
are not a crash journal. Full Notepad++ backup semantics, ACL-specific tests, and
filesystem coverage beyond the tested Windows filesystem remain required work.

Undo/redo stores reversible text changes and small metadata records rather than
full-document snapshots per edit. Batch replacement and line-ending conversion
retain only affected text. A test verifies 1,000 one-character replacements in
a 2 MiB document retain 1,000 bytes of history text payload (excluding allocation
and record overhead). The live document and savepoint still use full strings;
Line positions are cached; single edits update the affected line range while
batch edits rebuild the index lazily. Tests compare Unicode and CR/LF boundary
edits against an independent full scan, including undo/redo. Updating offsets
after an edit still takes linear time in the number of later lines.
Large-file performance still needs a scalable text buffer and
a view that avoids copying the whole document before claiming editor parity.

## Licensing

The rewrite is GPL-3.0-or-later. See `LICENSE` and the original notices in
`notepad_plus_plus/LICENSE`. The supplied Scintilla, Lexilla, and other bundled
components retain their own licenses. This project is an independent rewrite.
