# Feature parity evidence

Full Notepad++ reproduction is the completion criterion. This list is a starting
inventory, not a reduced definition of that goal. Native-control integration
tests cover basic Windows editing; full visual and interactive parity is pending.

| Area | Source reference under `notepad_plus_plus/` | Current evidence / remaining work |
| --- | --- | --- |
| Menu commands | `PowerEditor/src/menuCmdID.h` | Constants generated for all active defines; behavior dispatch pending |
| Document editing | `PowerEditor/src/ScintillaComponent/` | Unicode edits, atomic batches, history, case conversion, duplicate caret line, selection-aware leading/trailing trimming and EOL conversion tested; Windows menu integration tested. Renderer, advanced selections, remaining editing commands, locale case parity and scalable storage pending |
| File operations | `PowerEditor/src/Notepad_plus.cpp` | Native dialogs, metadata-preserving replacement with recovery backup, Save As open-tab protection. Tests cover locked/read-only failures, stream/creation-time preservation, new-file race and simulated partial recovery; ACL-specific tests, filesystem coverage, external-change detection, Notepad++ backup policy and crash recovery pending |
| Search | `PowerEditor/src/ScintillaComponent/FindReplaceDlg.cpp` | Native modeless Find/Replace, forward/backward wrap, case and whole-word options, F3 navigation and transactional replace-all; real Find button/close lifecycle tested. Regex, configurable word classes, marking, directory search and full Notepad++ dialog pending |
| Syntax | `lexilla/` and `PowerEditor/src/ScintillaComponent/` | Pending Rust lexer/style/folding implementations |
| Settings | `PowerEditor/src/Parameters.cpp` | Pending preferences, language definitions, themes and shortcuts |
| Plugins | `PowerEditor/src/MISC/PluginsManager/` | ABI layout tests and a Windows DLL loader tested with compiled fixtures. Application integration, NPPM/Scintilla messages, lifecycle notification dispatch, docking and administration pending |
| Sessions and views | `PowerEditor/src/Notepad_plus.cpp` | Native tabs, dirty indicators, Save All, Reload, group close scopes and cancellation tested. Tab navigation/reordering retain identity; Restore Last Closed File reads from disk. Reload maps Unicode positions; multi-view, persisted/configurable recent files, sessions and recovery pending |
| Macros and commands | `PowerEditor/src/NppCommands.cpp` | Pending command dispatcher, recording, playback and Run integration |
| Auxiliary panels | `PowerEditor/src/WinControls/` | Pending project, folder, document map, function list and other panels |
| Printing and OS integration | `PowerEditor/src/` | Explorer file drops snapshot Unicode paths and open multiple files through the normal loader; shell HDROP and duplicate-tab handling tested. Folder drops, printing and remaining OS integration pending |
| Localization/accessibility | `PowerEditor/` | Pending |
| Platforms | Workspace root | Windows/MacOS/Linux directories present; Windows executable builds; other adapters and packaging pending |

Undo history now uses reversible changes rather than full-document snapshots.
Explicit legacy decoding/conversion covers Windows-1252, Windows-1251 and
Shift-JIS using portable Rust encoding tables. Tests cover known byte sequences,
malformed sequences, nonrepresentable text, writer refusal before mutation,
reload and Windows view integration. Automatic detection, system ANSI behavior,
remaining code pages and persistence across application restarts remain pending.
In-memory recent files retain the saved encoding, including when an unsaved
encoding conversion was discarded before closing.
Line-order commands include ascending/descending UTF-16 lexicographic sorting
and reversal. Tests cover selected-line boundaries, final empty lines, Unicode
ordering, native command dispatch, undo, and read-only state. Other sort modes
and rectangular-column sorting remain pending.
Tests cover sparse edits in a 2 MiB document, adjacent batch edits, redo branches,
metadata-only changes, and sparse replace-all. This does not establish overall
large-file performance: live text, savepoints, indexing and the native-view bridge
still need scalable implementations.
Line indexing now reuses a cache for repeated queries and locally rescans after
single edits. Tests compare Unicode and mixed-ending boundary edits with an
independent scanner, including undo/redo, batch invalidation and reload. Bulk
rebuilds and shifting later line offsets still have linear costs.

Pre-save disk-content checks detect changed/deleted current files and require
an overwrite decision. Tests exercise real-file refusal, confirmed overwrite,
deletion/recreation, failed-save baseline retention and encoding-byte changes.
Automatic file watching and closing the check-to-write race remain pending.

Plain-text matching-brace navigation and inclusive pair selection are available
through Search. Tests cover nesting, Unicode offsets, caret priority, unmatched
braces, read-only navigation and native selection. Lexer-style filtering and
automatic highlighting remain pending.

Invert Case now uses the shared Unicode conversion path with one undo transaction.
Tests cover expanding mappings, titlecase characters, backward selections,
unchanged text, read-only failure and Windows menu dispatch. Like upper/lowercase
conversion, Rust Unicode mappings can differ from upstream Windows locale APIs.

Explicit BOM-less UTF-16 LE/BE opening and conversion preserve byte order through
save, reload and recently closed files. Known-byte, surrogate, odd-length input,
history and native lifecycle tests cover both variants. Automatic detection of
BOM-less UTF-16 remains pending; explicit decoding treats every code unit as text.

Whitespace conversion includes TAB to Space and leading/all Space to TAB using
the supplied wsTabConvert column and whitespace rules. Core tests cover Unicode,
mixed endings, boundary spaces, selection expansion, caret mapping and undo;
Windows command dispatch is tested with real controls. Commands use the session's
selected tab width. Leading-only conversion preserves upstream's existing-tab boundary.

Extended search decodes control and fixed-width numeric escapes in both find
and replacement text, preserving unknown/incomplete escapes. Tests cover UTF-16
surrogate pairs, multiline/backward search, byte offsets, decoding exactly once,
undo and native request dispatch. Unpaired surrogates are rejected rather than
introduced into the UTF-8 buffer. The native view rejects NUL replacement text.
Mode selection currently lives in the Search menu; the upstream search dialog
layout and regex engine remain pending.

Consecutive and all-duplicate line removal now have shared-core implementations
and Windows menu commands. Tests cover Unicode, case sensitivity, mixed endings,
empty lines, unterminated tails, partial selections, read-only failure and undo.
Their different EOL handling follows removeDuplicateLines and
ScintillaEditView::removeAnyDuplicateLines in the supplied source. Full upstream
selection/caret behavior still needs side-by-side GUI verification.

Find navigation now scans from the caret in the requested direction and stops
at the selected match, without allocating all document matches. Count uses
non-overlapping occurrences, and Replace All builds edits directly from the
iterator. Boundary tests compare both directions and wrap settings against all
match ranges across Unicode, overlapping and whole-word cases. Native tests
verify Count reads edited dialog controls and preserves selection. Overall
large-file performance remains pending.

Count and Replace All support an explicit UTF-8 range in the core and a current
selection toggle in the Search menu. Tests cover complete-match containment,
surrounding word boundaries, Unicode and mixed endings, empty/invalid scopes,
read-only rejection, length-changing replacements and selection restoration on
undo/redo. Native tests verify UTF-16 control positions map to the intended range.

The Windows plugin loader resolves all six required exports, checks Unicode mode,
initializes plugin data and command arrays, copies menu metadata, assigns command
IDs and exposes invocation/message/notification callbacks. Compiled Rust DLL
fixtures exercise success, separators, ID writeback, missing exports, ANSI mode,
invalid command counts and unloading after success/failure. Notification layout
tests cover the supplied SCNotification x64 ABI. Loading remains an unsafe API
for trusted ABI-conforming DLLs; fixtures do not prove real-plugin compatibility.
The editor does not yet discover or load installed plugins.

Plugin registration reserves the upstream 22000–22999 command range, assigns
only actionable menu entries, writes IDs into plugin-owned arrays and routes
callbacks through retained DLL ownership. Tests compare the reservation against
resource.h and exercise two independent DLLs, separators, checked metadata,
notification broadcast, unknown commands, duplicate modules and range exhaustion
at the u16 boundary. Rejected registrations retain the loaded DLL for caller-led
shutdown. A compatible NPPM/Scintilla host remains pending.

The Windows Plugins menu now renders registered commands, checked states and
separators, with WM_COMMAND dispatch through the registry. A compiled native
fixture verifies routing, menu rebuilding, old-menu destruction, shutdown event
delivery while the main window is alive and subsequent unloading. Popup replacement
uses [ModifyMenuW's documented ownership behavior](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-modifymenuw).
The app does not yet discover/load third-party plugins, register their keyboard
shortcuts or expose a compatible NPPM/Scintilla message surface.

Seven synchronous NPPM queries now report current buffer identity, positions,
open-file counts, encoding and EOL format from per-window snapshots. Definitions
are checked against Notepad_plus_msgs.h. Native tests cover reorder/close/reopen,
invalid IDs, hidden secondary view, conversion updates and queries made during a
DLL command callback. Opaque token allocations survive closed buffers until host
teardown to prevent stale-ID reuse. Only the main view exists; writable messages,
remaining queries, Scintilla APIs and general plugin compatibility remain pending.

Six file-path queries extend the snapshot host: full path by buffer ID and the
current path/directory/filename/name/extension family. Native tests verify UTF-16
lengths with non-BMP names, terminators, untouched guard values, insufficient
capacity, null pointers and the legacy zero-capacity convention. Path components
use the same Windows shell helpers as the supplied implementation. Invalid IDs
return zero for GETFULLPATHFROMBUFFERID, matching NppBigSwitch.cpp despite the
header's documented -1. General plugin compatibility is still unproven.

Untitled names now belong to workspace buffers rather than tab positions.
Allocation follows FileManager::nextUntitledNewNumber: use the lowest available
positive number and release it after save/close. Tests cover reordering, number
reuse, failed save/close, named files and agreement among native tabs, titles and
plugin filename queries. Localized names and session-restored names remain pending.

Caret line/column and current-line-text plugin queries now share portable line
range and column calculations with the status bar. Tests cover tab stops, Unicode
scalars and combining marks, CRLF positions, invalid offsets, edit/undo indexing,
native selection updates, UTF-16 output bounds and the upstream 2048-byte line
limit. View now selects session-wide tab widths of 2, 4 (default), or 8, shared
by native rendering, conversions, status and plugin column queries. Native tests
compare tab and space pixel positions for each width with the current fixed font,
check menu selection, preserve selection and history, and verify conversion undo.
Persisted settings, per-language overrides and configurable fonts remain pending.

Validation must eventually include side-by-side upstream behavior fixtures,
real plugin compatibility tests, GUI interaction, large files, disk failures,
encoding coverage and release builds. Core unit tests alone cannot prove parity.

Stream indentation follows `scintilla/src/Editor.cxx::Indent`,
`Document.cxx::Indent/SetLineIndentation`, and the single-line menu override in
`PowerEditor/src/NppCommands.cpp` and `ScintillaEditView.cpp::setLineIndent`.
Tab/Shift+Tab and Increase/Decrease Line Indent use the selected width and the
session's tabs/spaces preference. Tests cover selection replacement, leading whitespace,
Unicode-safe back-tab movement, mixed endings, empty-line skipping, reversed
core selections, endpoint exclusion, read-only failures and grouped native undo.
Spaces mode also tests inline tab-stop insertion after Unicode and selection
deletion, mixed leading whitespace, retained inline tabs, native preference
toggling and undo/redo. Preference changes do not modify existing text.
Persisted and per-language preferences, independent indent width, rectangular/multiple
selections and native reverse-selection preservation remain pending.

Delete Line and Transpose Line follow `scintilla/src/Editor.cxx::LineDelete`
and `LineTranspose`, with the supplied Ctrl+Shift+L and Ctrl+T shortcuts.
Tests cover caret-line targeting across selections, mixed endings, Unicode,
final unterminated/empty lines, first-line transpose, read-only protection,
selection mapping and native undo. Native reverse selections and multiple carets
still require the advanced selection implementation.

Delete to Start/End of Line follows `Editor.cxx::DelWordOrLine` for
`SCI_DELLINELEFT/RIGHT`, including caret-based deletion, rightward selection
collapse and leftward endpoint mapping. Core and native tests cover Unicode,
CR/LF/CRLF boundaries, no-op undo history, read-only failures and selection
restoration. Virtual space and multiple selections remain pending.

Remove Empty Lines and Including Blank Characters implement the two-pass text
rules in `Notepad_plus.cpp::removeEmptyLine` without a regex dependency. Tests
cover exact selection limits, ASCII blanks versus nonbreaking spaces, mixed
endings, trailing empty lines after first-pass removal, Unicode, read-only
protection and native undo. The rewrite groups both passes into one transaction;
full regex-engine and GUI selection fidelity still need side-by-side verification.
Empty-line scanning now skips consumed newline runs and merges adjacent removal
ranges during traversal, avoiding quadratic rescanning and a separate sort.
Regression coverage includes 100,000-line empty/blank runs and 25,000 separated
blank lines with undo/redo. Document rebuilding still copies the text; this does
not establish overall large-file renderer or storage parity.

Join Lines follows `NppCommands.cpp::IDM_EDIT_JOIN_LINES`,
`ScintillaEditView.cpp::getSelectionLinesRange` and `Editor.cxx::LinesJoin`.
Core/native tests cover mixed endings, Unicode, retained indentation/tabs,
consecutive empty lines, selection expansion and endpoint exclusion, read-only
protection and undo/redo. Ctrl+J is wired to the Windows command. Multiple and
rectangular selections remain pending.

Insert Blank Line Above/Below follows the corresponding `ScintillaEditView.cpp`
helpers and their configured EOL-width arithmetic. Tests cover CRLF/LF/CR,
first/last/empty documents, Unicode, selection collapse, read-only failures,
undo/redo and native caret placement. Mixed-EOL offsets that would split UTF-8
or fall outside the document fail atomically rather than corrupting text;
mixed-EOL behavior still needs side-by-side upstream GUI verification.

Enter now bypasses native text diffing and inserts the configured EOL through the
core. Optional Basic Auto-Indent follows the basic branch of
`Notepad_plus.cpp::maintainIndentation`, including beginning-of-line exclusion,
existing indentation and the tabs/spaces preference. Tests cover CR/LF/CRLF,
Unicode selections, grouped undo, read-only failure and native Enter dispatch.
Language-aware indentation, persisted mode selection and notification/plugin
integration remain pending.

Move Lines Up/Down follows `Editor.cxx::MoveSelectedLines` for stream blocks:
whole-line expansion, endpoint exclusion, moving the final empty line, and
configured endings at the unterminated tail. Tests cover mixed endings, Unicode,
boundaries, read-only failure, reversed core output selections and native undo.
The native EDIT control still loses selection direction when reading selection
endpoints; rectangular/multiple selection and folded-line movement remain pending.

Windows command dispatch now gates text/encoding mutations on the active
buffer's read-only state and updates editing menu availability. Undo/Redo also
reflect history availability. A native test dispatches every gated command,
checks unchanged text/selection, exercises Back-tab navigation and verifies
menu updates after switching to a writable buffer. Replace-dialog buttons now
track the active buffer and nonempty query, including after native edit-control
notifications. Replacement dispatch separately rejects read-only mutations.
A native test covers query edits, buffer switching, empty-query disablement,
queued requests and an actual Replace All button click. Clipboard-dependent
menu state and read-only plugin notifications remain pending.

User and file read-only flags are now independent, matching `Buffer::isReadOnly`
and `NppCommands.cpp`'s user-toggle/all-documents commands. Core/native tests cover
combined protection, menu checks, native edit state, history preservation and
reload. Reload refreshes only the file flag; successful Save As clears file
protection while preserving the user flag. The system file-attribute toggle now
preserves unrelated Windows attributes, updates buffer state only on success,
and is disabled for untitled buffers. Native/filesystem tests verify content,
hidden/archive flags, user protection and history remain intact. Directory,
missing-file and NUL paths are rejected. Upstream's access-denied elevation
workflow remains pending. Activation now starts a background metadata check for
open files, applying results only to matching buffer IDs and paths. Save, reload
and explicit attribute changes invalidate pending results. Tests verify that
external attribute changes preserve unsaved text, selection and undo history,
and stale paths are ignored. Inaccessible paths retain prior protection state;
background streamed fingerprints now identify byte changes, deletion and
inaccessible paths in the status bar. Comparison uses loaded/saved bytes, so
unsaved edits do not trigger false external-change reports. Tests cover same-size
changes, deletion, path replacement with a directory, restoration and explicit
reload, preserving dirty text until reload. Continuous watching, cancellation of
blocked filesystem calls, upstream notification dialogs and full file-state
handling remain pending.

Invalidated disk checks now signal cancellation before further files and between
64 KiB hashing reads. The owned check handle cancels on drop, including window
shutdown; cancelled partial results are not published. Core tests verify digest
equivalence and cancellation before opening and between chunks. Cancellation
cannot interrupt an operating-system call already blocked on a remote path.

Detected deletion now marks a named buffer as needing a save independently of
text history, following the retained-copy protection in `Buffer::checkFileState`.
It clears file read-only protection while retaining the user flag. Dirty title/tab
indicators and close safeguards update without changing text. Tests cover a
previously clean deleted file, undo, failed save, successful save and reload.

Batches with one effective text change now use localized String replacement and
cached line-index updates instead of constructing a second complete document.
Validation and batch selection mapping remain unchanged. Regression tests cover
CRLF splitting/merging, Unicode replacements, undo/redo and a 100,000-line buffer
with no-op entries and one effective edit. String suffix movement and native-view
projection costs remain; this is not a claim of full large-file parity.

Windows Zoom In/Out/Restore commands use the supplied numeric-keypad shortcuts
and -10..60 zoom bounds. Owned scalable fonts are replaced/released on the UI
thread; restore selects the original stock font. Native tests check increased
glyph spacing, aligned tabs/spaces, retained margins/selection/text and unchanged
undo history. Ctrl+mouse-wheel zoom now follows the supplied wheel accumulator
and one-step-per-message rule, including partial deltas and the disabled-wheel
setting. Native tests cover both directions, multi-notch messages, ordinary
scrolling bypass and Shift exclusion. Shift-wheel and native horizontal-wheel
input scroll sideways with the supplied direction rules and Windows character
scroll setting. Native tests cover partial deltas, both directions, Shift taking
precedence over Ctrl, and unchanged selection, text and undo history.
Typography, DPI transitions, per-view zoom and
font preferences still require implementation or upstream parity verification.

Go To (Ctrl+G / IDM_SEARCH_GOTOLINE) now opens a modeless Windows dialog with
line/offset modes, current position and maximum, submit and cancel. The portable
core handles one-based lines and UTF-8 byte offsets, clamping ranges and rounding
offsets forward out of Unicode characters and CRLF pairs. Decimal-prefix parsing
and the source dialog's -1 sentinel follow GoToLineDlg.h; navigation rules follow
GoToLineDlg.cpp and Scintilla's GoToLine/PositionBefore/PositionAfter. Core and
native tests cover mixed endings, Unicode, read-only navigation, control caret
mapping, mode indicators, cancellation and unchanged undo history. This dialog
does not yet reproduce upstream theming, localization, DPI adaptation or folded
line expansion. Offsets refer to the core's UTF-8 text; matching upstream offsets
for legacy-codepage documents still requires the encoding/view compatibility work.

SCI_LINECUT and SCI_LINECOPY equivalents are exposed as Cut Lines (Ctrl+L) and
Copy Lines (Ctrl+Shift+X), using the supplied Parameters.cpp bindings. They include
the line at the selection end, even at an exact line-start boundary, and preserve
original mixed line endings and unterminated final lines. Copy retains selection;
cut collapses at the removed range start and undo restores the expanded selection.
The Windows adapter publishes CF_UNICODETEXT with movable-memory ownership transfer
and frees allocations on failure. Failed clipboard publication does not remove
text. Tests exercise core range/history semantics, native UTF-16 selection mapping,
read-only behavior, injected clipboard failures, and the allocated Unicode payload
without replacing the user's clipboard. Real cross-application clipboard paste
verification remains pending. Ordinary empty-selection copy/cut, rectangular copy,
line-copy clipboard markers and line-aware paste remain separate unfinished work.

Copy All File Names/Paths now use the upstream command IDs and buf2Clipboard
ordering/terminator rules: tab order, untitled names included, CRLF after the final
entry. Clipboard encoding retains native Windows path units without a lossy string
conversion. Tests cover reordering, closing tabs, Unicode names, dirty/read-only
buffers, stable untitled names, and unpaired UTF-16 path units. Cloned buffers in a
second view and Document List panel selection remain pending with those features.

Save a Copy As uses IDM_FILE_SAVECOPYAS and preserves document identity, savepoint,
selection, history and read-only flags through an immutable portable-core method.
The Windows menu opens a titled save dialog with overwrite confirmation and writes
through the existing atomic writer. Save/Save As/Save Copy share destination checks
for other open buffers and external changes to the source path. Tests cover named
and untitled documents, UTF-16 BE output, write failures, savepoint preservation and
open-tab protection. Upstream's optional Open Copy After Saving checkbox, remembered
dialog settings, file-type filters and plugin save notifications remain pending.

Case-insensitive and line-length ascending/descending sorts now use their upstream
menu IDs. Case keys reproduce _wcsicmp's default C-locale ASCII folding and UTF-16
ordering, including NUL termination; the supplied application has no setlocale call.
A Windows differential test compares representative keys against the actual CRT.
Length keys count UTF-16 units as Sorters.h does. Cached keys avoid recomputing them
for every comparison. Tests cover selected ranges, mixed endings, supplementary
characters, undo, no-op orders and read-only command guards. Equal keys retain input
order here; upstream's whole-line std::sort does not promise stable ties. Exact
tie permutations, plugin-induced CRT locale changes, rectangular column sorting,
numeric sorts and user-locale sorting still require parity work.

Integer ascending/descending sorting now ports the whole-line IntegerSorter
comparator from Sorters.h. It compares numeric chunks by sign, significant digit
count and digits, then breaks equal numeric values by leading-zero count before
continuing through text. This avoids numeric overflow and retains the source's
special handling of non-numeric minus signs. UTF-16 keys are built once per line.
Tests cover multiple numeric chunks, long numbers, negative zero, leading zeros,
Unicode prefixes, mixed endings, undo and read-only Windows commands. This supersedes
the integer-sort portion of the pending numeric work above; decimal-dot/comma,
rectangular keys and exact equal-key permutations remain unfinished.

Decimal-dot/comma ascending/descending commands now prepare numeric prefixes using
the supplied admissible-character rules and stable-sort finite f64 keys. Empty
prepared keys appear first ascending and last descending; equal values retain
their order. Conversion errors report the one-based document line and leave text,
selection and history intact. Tests cover both separators, suffix truncation,
empty keys, negative values, equal values, conversion failure, extreme overflow/
underflow, undo and native command/read-only routing. This supersedes the pending
decimal commands above. Exact floating-point rounding and subnormal range behavior
still need differential verification against _wcstod_l; rectangular keys remain
pending with rectangular selection support.

A Windows differential boundary test now checks decimal conversion acceptance and
both sort directions against _wcstod_l with the same en-US numeric locale as the
source. It exposed and fixed rejection of representable subnormal values: these
now sort successfully, while nonzero values rounded to zero still report a range
error. Cases include the smallest subnormal, the normal/subnormal boundary, maximum
finite value, overflow and underflow. This verifies those sampled boundaries;
exhaustive decimal-rounding equivalence remains unproven.

The shared sorting range implementation now follows ScintillaEditView::sortLines:
it extracts the selected whole-line block, splits on the configured EOL string,
sorts those items, and rejoins with that delimiter. It preserves the boundary
separator when the range stops before the document end and includes the final
empty item when sorting through EOF. Earlier claims of retaining mixed line-ending
slots describe the superseded implementation. Mixed endings now travel inside
sort items, as in the source, rather than each forming an independent key. Tests
cover full and selected mixed-ending blocks, delimiter mismatch, trailing empty
items and undo. Ordinary LF sort fixtures now explicitly configure LF before
inserting their text instead of silently relying on a CRLF new-buffer default.

Short/long date-time insertion now uses the source command IDs and Windows
GetLocalTime/GetDateFormatEx/GetTimeFormatEx with the user's regional settings and
TIME_NOSECONDS. Default order is time then date; a session-only menu option reverses
it. A selection is replaced in one undo transaction, including supplementary
Unicode selection mapping, and read-only buffers reject the command. Tests cover
both live commands, undo, preference state, seconds omission and invalid dates.
Customized format parsing, the upstream preferences UI and persistence remain
pending; those features are not represented by the short/long commands.

The custom date/time command now uses the source default yyyy-MM-dd HH:mm:ss.
Its formatter ports Common.cpp's time-then-date Win32 formatting and temporary
AM/PM token escaping, including single/pair token restoration, empty locale
designators, the 260-unit buffer limit and empty output on formatting failure.
Tests cover fixed date/time output, seconds, AM/PM, empty/oversized formats and
the live custom command's selection/undo/read-only behavior. The application
holds a format string ready for preferences; its editing UI and persistence are
still missing, so users currently have the default custom format only.

Legacy plugin GETOPENFILENAMES, GETOPENFILENAMESPRIMARY and GETOPENFILENAMESSECOND
messages now follow NppBigSwitch.cpp's path-array contract and return count. The
single-view host enumerates primary tabs in order, includes untitled names and
returns zero for the secondary view. Caller entry counts bound the number copied;
as in the original API, each individual string requires caller-allocated capacity
for its full path plus NUL. Snapshot paths are cloned before writing caller memory.
Native message tests cover zero/null requests, limited and oversized entry counts,
Unicode, terminators, untouched surplus storage and tab reordering. This extends
the read-only query subset; general third-party plugin support remains incomplete.

NPPM_MODELESSDIALOG now registers and removes plugin-owned dialog handles with the
source's duplicate/absent-handle return values. The message loop dispatches through
IsDialogMessage before application accelerators. Handle snapshots release TLS
borrows before callbacks, and registration is rechecked between dispatches so a
callback can remove another dialog. Invalid/destroyed handles are skipped; removing
registration never destroys plugin windows. A native test exercises Escape routing,
duplicate registration, snapshot refresh, reentrant host queries/unregistration,
absent removal and a destroyed registered window. Docking, plugin discovery and
broader Scintilla compatibility remain unfinished.

NPPM_GETMENUHANDLE now exposes the live main/Plugins menus, returning zero for an
unknown choice. NPPM_SETMENUITEMCHECK updates native check state and follows the
source's TRUE return for unknown commands. Runtime checks survive plugin-menu
rebuilding, and the published Plugins handle is refreshed after replacement.
The controlled loaded-plugin test covers both handles, invalid choices, checked/
unchecked states, unknown commands, rebuilding and snapshot refresh. Toolbar check
synchronization is pending with the toolbar implementation; these messages do not
establish general plugin compatibility.

NPPM_GETSHORTCUTBYCMDID now reads copied ShortcutKey metadata for registered plugin
commands. Unknown/zero commands, null output pointers and zero-key shortcuts return
FALSE; failures leave caller storage unchanged. Shortcut snapshots survive ordinary
document refresh and are republished with plugin menus. The loaded-DLL fixture now
provides Ctrl+Alt+R and verifies all ABI fields, failure cases and rebuild behavior.
User remapping, shortcut removal and actual plugin accelerator dispatch remain
pending, so this query currently describes the registered defaults.

Registered plugin defaults now also create an owned Windows accelerator table.
The message loop checks it after modeless dialogs and built-in accelerators;
matching keys dispatch through the existing queued WM_COMMAND route. Rebuilding
plugin menus prepares a replacement table before changing the live menu, and old
tables are released on replacement/drop. The loaded-DLL test exercises Ctrl+Alt+R
via TranslateAcceleratorW and verifies missing/extra modifiers do not dispatch.
It changes only the test thread's keyboard state and restores it before assertions.
User shortcut remapping/removal, shortcut labels and conflict UI remain pending.

NPPM_REMOVESHORTCUTBYCMDID now disables registered plugin defaults for the running
host. It returns FALSE for zero and TRUE for nonzero IDs, including unknown IDs,
as the supplied PluginsManager does. Queries stop returning the removed shortcut
immediately; accelerator translation rebuilds its table without disabled bindings,
and menu rebuilding does not restore them. The loaded-DLL test verifies removal,
unchanged query output on failure, rebuild persistence, keyboard non-dispatch and
continued manual menu invocation. Saving shortcut removal to shortcuts.xml remains
pending with preferences persistence and remapping.

Locale ascending/descending sorting now follows SortLocale's stream-selection
algorithm separately from ordinary line sorting. The portable core accepts sort
keys from its platform adapter; Windows uses LCMapStringEx with the source defaults
(user locale, linguistic case folding and digits as numbers). Whole line contents
carry their endings, the final empty line is excluded, an unterminated final line
transfers its ending, and stable ties retain input order. Unselected carets follow
their original line; selections expand to full lines and retain direction. Keys
are computed before mutation, and changes form one undo step. Portable tests cover
mixed endings, selection boundaries, ties, caret restoration and failure atomicity;
a native command test covers numeric/caseless ordering, both directions, read-only
protection and undo. Locale configuration UI, rectangular/multiple selections and
legacy-codepage byte-position compatibility remain pending.

Random Line Order dispatches the source command ID through ordinary sorting's
configured-EOL splitting, selected-line boundaries and one-step undo. The core
reproduces MSVC v143's MT19937, bounded rejection sampling and forward shuffle;
the Windows adapter supplies epoch seconds truncated to unsigned, as the source
does. Tests compare 40 permutations against a C++ fixture compiled with the local
14.44.35207 toolset, including seeds zero/UINT_MAX and 625/1000-item sequences.
Another 18 fixture cases exercise rejection and 64-bit bounds. The fixture source
and generated values live in crates/editor-core/tests/fixtures; ordinary cargo
tests do not need a C++ compiler. Other C++ standard-library versions are not
verified. Integration tests cover repeatability, preservation of duplicates/
Unicode/final empty entries, selected mixed-EOL blocks, read-only protection and
native command dispatch.

Proper Case and Sentence Case now have Force/Blend commands under their source IDs.
The Windows adapter follows changeCase's UTF-16 unit loop and native character
classification/mapping, including straight/both smart apostrophes, numeric word
prefixes, adjacent punctuation, paragraph breaks and the English pronoun exception.
Document::transform_selection supplies a portable single-edit transaction and
restores a forward result selection. Native tests cover these rules, preserved
surrogates/NUL, all four menu commands and read-only guards. A core test covers
reversed selection undo; the temporary native EDIT renderer normalizes direction
when synchronizing its selection, so that UI behavior remains a renderer gap.
Multiple/rectangular selections and legacy-codepage byte-position parity remain
pending; Upper/Lower still use their earlier Rust Unicode mapping.

Invert Case now uses IsCharLowerW and CharUpperW/CharLowerW per UTF-16 unit, fixing
the earlier Rust Unicode expansion and supplementary-character mapping differences.
Random Case uses the source command ID, IsCharAlphaW and the CRT rand stream,
drawing only for alphabetic units and retaining the stream across calls. Tests
cover sharp-s, accented letters, supplementary characters, NUL, nonletter draw
skipping, known CRT draws on an isolated thread, native commands, undo, empty
selections and read-only protection. Quote/easter-egg features that also reseed
the source CRT stream are not yet implemented.

NPPM_GETNPPFULLFILEPATH and NPPM_GETNPPDIRECTORY now return the running host's module
path and directory. They use GetModuleFileNameW and PathRemoveFileSpecW with the
source's MAX_PATH buffer, enforce capacity including the UTF-16 terminator, and
support the legacy zero-capacity convention. Null output returns FALSE. Native
tests verify the actual executable location, short/exact capacities, untouched
failure buffers, terminators/guard units and independence from the active file.
This retains the source's path-length limit; settings/plugin configuration path
queries remain pending with configuration and plugin discovery.

Uppercase and Lowercase now also use the Windows UTF-16 unit adapter. Inspection
of ScintillaEditView.h confirmed that supported Windows versions call changeCase;
SCI_UPPERCASE/SCI_LOWERCASE are only the old-OS fallback. This fixes Rust expansion
of sharp-s/ligatures, contextual final-sigma conversion and supplementary-letter
mapping differences. Native tests exercise these cases, dotted/dotless I, accented
letters, NUL, selected substrings, empty selections, read-only guards and undo.
The portable core's explicit Unicode change_case API remains available, but Windows
menu commands no longer use it. Multiple selections and legacy-codepage conversion
remain outstanding.

Current full-path, filename and directory clipboard commands now use their source
IDs and CF_UNICODETEXT. Filename extraction uses PathFindFileNameW; directory
extraction reproduces Common.cpp's pathRemoveFileSpec rather than the different
shell helper used by plugin queries. Native UTF-16 units survive unchanged,
including unpaired surrogates in filenames. Tests cover roots/UNC/drive-relative
paths, untitled names and empty directories, active-tab identity, Unicode, read-only
menu availability and writer failures without text/history/selection mutation.
Tests inject the writer so validation does not replace the user's clipboard.

Begin/End Select now has its stream-selection menu command and default shortcut.
A portable shared SelectionGesture holds the view's transient byte position;
Document updates it for single/batch edits and undo/redo after validation succeeds.
The update order and strict position < marker - 1 boundary follow the source.
The second invocation selects to the caret, clears the marker and menu check,
without adding undo history. Tests cover backward selection, read-only buffers,
history replay, failed edits, cross-document clamping and native menu state.
The UTF-8 core rounds an anchor inside a scalar forward to preserve its position
invariant. Rectangular mode and preservation of native EDIT selection direction
remain outstanding with the renderer work.

View Summary now reports source-style character, word, line, internal byte and
stream-selection counts. UTF-8 character counts exclude CR/LF; other encodings
follow the source's configured-EOL subtraction, including its UTF-16 selection
count multiplier. Legacy byte counts come from lossless codepage encoding; an
unrepresentable unsaved edit reports an error instead of inventing a byte count.
The fixed word delimiter set matches wordCount's regex, so underscores, NBSP and
form feed do not behave like generic Unicode word/whitespace boundaries. The
Windows dialog formats grouped counts and accessible saved-file timestamps in
the user's local timezone and regional date/time format. Tests cover all these
count paths, mixed endings, BOM exclusion, empty buffers and metadata formatting.
Multiple/rectangular ranges, unnamed backup metadata, localization and dark-dialog
styling remain pending. Non-Unicode filesystem path units currently use the same
lossy display conversion as the window title.

The status bar now includes selected characters/lines or an unselected one-based
byte position, plus the document line total. The core count follows
getSelectedCharsAndLinesCount: Unicode scalars count individually (including CR
and LF), while a selection ending exactly at a next-line start excludes that
line. Tests cover both selection directions, mixed endings, partial CRLF,
supplementary characters, combining marks, empty buffers and native status text.
The current renderer still has a single status label; multi-pane layout,
localization/grouped numbers and legacy-codepage byte-position parity are pending.

Open Containing Folder in Explorer and Open in Default Viewer now use the source
command IDs and Windows shell APIs. Reveal uses a COM PIDL and selects the file,
falling back to exploring its parent on file-not-found; default viewing uses the
registered open verb. Menu availability checks existing files and the source's
association query/exclusion rule. Results are cached between active-path changes,
successful saves and background attribute refreshes to avoid shell queries on
every keystroke. Tests inject launches to verify saved identity, unsaved/read-only
state preservation and errors, and separately verify COM MTA fallback and PIDL
parsing without opening external windows. Actual Explorer selection and associated
application launch have not been visually exercised in validation.

The Show Status Bar menu toggle shares
the native visibility path with NPPM_HIDESTATUSBAR/NPPM_ISSTATUSBARHIDDEN. The hide
message returns the previous shown state, treats only lParam == TRUE as hide,
and resizes the editor synchronously without borrowing App in the window procedure.
Tests check native visibility/menu checks, unchanged text/selection/undo,
read-only operation, repeated requests, nonboolean parameters and reclaimed editor
height. Settings persistence, fullscreen/post-it modes and final status-bar pane
layout remain outstanding. An Always on Top implementation was withheld because
native tests observed SetWindowPos succeeding without consistently setting
WS_EX_TOPMOST; that behavior still needs investigation before exposing the command.

Further topmost isolation found that direct native calls also leave the flag
unchanged until another native window is first made topmost. The window procedure
does not modify WINDOWPOS in this reproduction. No workaround is shipped; the
cause remains unresolved.

Show Tab Bar and NPPM_HIDETABBAR/NPPM_ISTABBARHIDDEN now share the native visibility
path. Unlike the status API, hiding tabs accepts any nonzero lParam and returns
the previous hidden state. Editor resizing combines both visibility settings
synchronously, including when the parent window is hidden. Tests cover repeated
calls, negative/nonboolean inputs, menu checks, combined reclaimed height, tab
creation/navigation while hidden, and unchanged document undo history. Preference
persistence and multiple editor views remain pending.

Show Menu Bar (rewrite shortcut Ctrl+Alt+M) and NPPM_HIDEMENU/NPPM_ISMENUHIDDEN now
detach/restore the native menu. Only lParam == TRUE hides; the hide call returns
the prior hidden state. The host retains the main handle for queries, checks,
command enablement and plugin-menu rebuilding while detached. Shutdown releases
a detached menu after plugin notifications; attached menus remain window-owned.
Native tests cover repeated requests, nonboolean input, restoration, read-only
enablement and plugin menu operations while hidden, plus detached-menu destruction.
Hidden menus now attach temporarily during the normal native menu loop and
detach on exit, preserving the hidden preference. System-menu entry (Alt+Space,
application icon and title-bar right click) and popup-menu loops are excluded as
in NppBigSwitch.cpp. Tests send lifecycle messages to a native window and cover
the temporary visible query state, unchanged menu preference, snapshot refresh,
system/popup exclusions and subsequent explicitly visible operation. Physical
Alt/F10 keyboard navigation has not been visually exercised. Visibility
persistence and localization remain pending.

NPPM_GETCURRENTVIEW and NPPM_GETCURRENTSCINTILLA report the main editor view.
The latter writes a 32-bit int even on Win64, ignores wParam and returns FALSE
for a null output pointer, following NppBigSwitch.cpp. Tests check sentinel
bytes around the output, tab creation/reordering/navigation, and both queries
from a loaded native plugin callback. These queries identify the existing single
view; they do not supply Scintilla handles or implement the outstanding secondary
view and Scintilla message surface.

Whole-word literal search now follows Document::IsWordAt and default UTF-8
WordCharacterClass, including Unicode combining marks, punctuation/symbol runs,
and transitions to whitespace or format characters. The generated compact class
table derives from the supplied Scintilla Unicode 16.0 category data, with its
license retained; regenerate with scripts/generate-word-classes.ps1 and cargo fmt.
An exhaustive scalar check compares generated classes with the source table.
Search/count/replace tests cover combining accents, underscore versus Unicode
connector punctuation, emoji, joiners, whitespace and undo. Custom word classes
and source legacy-codepage byte classification remain outstanding. The current
word plugin query also remains pending: its no-selection case must synchronously
expand the selection, which the read-only snapshot API does not yet implement.

Ctrl+Backspace/Delete now dispatch to Rust word deletion rather than the temporary
native EDIT behavior. Document::NextWordStart rules skip spaces and consume class
runs in the source's direction-specific order, including punctuation, combining
marks and adjacent newline characters. Commands operate from the caret even with
a selection; left deletion maps both endpoints and right deletion collapses them.
Core tests cover CRLF runs, format characters, selected text, undo/redo, empty
documents and read-only protection. A native-window test covers UTF-16 selection
projection and core history. Multiple selections, virtual space and legacy-codepage
classification remain pending.

Ctrl+Left/Right and Ctrl+Shift+Left/Right now use Rust NextWordStart navigation.
Non-extending movement starts from the caret, even with a selection; extending
movement retains its anchor. Navigation leaves text, dirty state and history
unchanged and works read-only. Native synchronization preserves known direction
when EM_GETSEL reports an unchanged sorted range, including key-up events after
backward extension. Core and native tests cover repeated movement, reverse
selection, Unicode projection, punctuation, CRLF and read-only operation.
Native-origin selections still cannot reliably report direction through EM_GETSEL;
rectangular/multiple selections, virtual space and folded-line visibility remain
outstanding. These limitations require further renderer work.

Commands now share the same direction-preserving native-selection synchronization
as word navigation. This covers editing, case conversion, line clipboard actions,
indentation, search setup, brace navigation, tab changes, date insertion, summary
and Begin/End Select. An unchanged native range retains the core anchor/caret;
a changed range is imported. Native text-change events still import their new
selection directly. A sequence test verifies backward word selection followed by
left/right word deletion, undo, case conversion and Begin/End Select. The existing
case-conversion test now requires undo to restore its original backward selection.
This retains known core direction but does not recover direction for mouse-created
native ranges, which remains a renderer limitation.

Home and Shift+Home now implement Scintilla VCHomePosition in the Rust core:
toggle between the first non-space/tab position and the current line's start,
retaining the anchor for the extending variant. Tests cover indentation, blank
lines, mixed endings, Unicode text, non-ASCII whitespace, read-only navigation,
unchanged dirty/history state and native UTF-16 projection. Wrapped/folded-line
visibility and rectangular selection remain pending with the final renderer.

End/Shift+End and Ctrl+Home/End (with or without Shift) now dispatch to Rust
line-end/document-boundary navigation. Physical line ends exclude CR/LF bytes;
document end includes a trailing newline. Extended movement preserves the anchor
and ordinary movement collapses at the target. Core tests cover mixed endings,
a caret inside CRLF, empty/final-empty lines, read-only and unchanged history;
native tests verify Unicode projection and reversing selection across commands.
Display-line variants, folding, rectangular selection and full scrolling-policy
parity remain renderer work.

Ctrl+[/] and their Shift variants now implement Scintilla ParaUp/ParaDown through
Rust, using the default Notepad++ OEM-key bindings in Parameters.cpp. Only empty
or ASCII-space/tab-only lines separate paragraphs. Up distinguishes a caret at
line start from one inside the line; down skips the current paragraph and its
following blank lines. Tests cover mixed endings, Unicode whitespace, empty
documents, boundaries, read-only history, selection anchors and native Unicode
projection. Folded-line skipping and rectangular selection remain pending.

Word-part navigation now ports Document::WordPartLeft/Right using ASCII case,
digit, punctuation and whitespace groups, underscore separator handling and
non-ASCII runs. Ctrl+OEM_2/OEM_5 and Shift variants match Parameters.cpp defaults.
Tests cover camel case, acronym boundaries, directional underscore behavior,
non-ASCII text, CRLF, NUL, document edges, read-only state and native selection
projection. Custom word classes, legacy byte encodings, folding and rectangular
selection remain outstanding.

Rust word, word-part, paragraph, Home, End and document-boundary navigation now
updates only the native selection/caret and caption/plugin snapshot. It no longer
rebuilds the UTF-16 projection, resets the edit text with WM_SETTEXT, or rebuilds
tabs on each navigation command. A native regression test preserves the edit's
modify marker (which WM_SETTEXT would reset), retains the projection allocation,
and checks current-line/column plugin queries after a command sequence. This
removes full text replacement during navigation; status/snapshot computation and
large-file rendering still require further performance work.

Find match selection, Go To line/offset, matching-brace navigation/selection and
Begin/End Select now also use selection-only native updates. Replace retains its
full text refresh before finding the next match. A native sequence test exercises
Find, Go To, braces and selection gestures in a read-only Unicode document while
checking that the projection and native edit modify marker survive unchanged.
All existing search/replace and navigation tests continue to pass.

Temporary selected-text search (IDM_SEARCH_VOLATILE_FINDNEXT/PREV) now uses the
source Ctrl+Alt+F3/Shift bindings. Empty selections expand using default UTF-8
SCI_WORDSTART/ENDPOSITION rules, including combining marks. Matching is literal,
case-insensitive, not whole-word, and wraps without replacing saved search text
or options. Tests cover word edges, punctuation, reverse selections, read-only
operation, forward/reverse wrapping and saved-option preservation. Full Unicode
case folding, custom word classes and the source wrap-status messages remain
pending.

Ctrl+F3/Ctrl+Shift+F3 now implement set-and-find: expand an empty selection, save
the query when it is at most the source default 1,024 characters, update an open
Find/Replace field and search with current options but Normal mode for this call.
The saved mode is retained. Native tests cover literal backslash-n while Extended
mode is selected, reverse search, live dialog synchronization and Unicode character
thresholds. The common-dialog shell does not create the source's hidden search
dialog on first use; search history, configurable thresholds and wrap-status
messages remain pending.

Search wrap feedback now reports the source's end/top-reached text in the main
status control for ordinary, set-and-find and temporary selected searches.
The next normal match restores document status. Native tests cover both directions,
Unicode match offsets, unchanged selection on wrap-disabled failure, and temporary
search wrapping independent of saved options. The final Find-dialog status pane,
localization and source visual feedback styling remain outstanding.

Reopening the same Find/Replace mode now reuses its live dialog, preserving
unsubmitted fields and options when there is no eligible editor selection.
Opening search follows the source defaults for expanding a caret word and filling
Find What with at most 1,024 characters. New Find dialogs honor the saved search
direction. Switching dialog modes carries the current query, case and whole-word
options into the new common dialog. Native tests cover handle reuse, live query
preservation, caret expansion and initial backward direction; count/read-only
tests now measure selection preservation after the expected opening expansion.
Unified Find/Replace tabs and configurable Find-field preferences remain outstanding.

Replacement text now survives switches between Find and Replace, including
unsubmitted Unicode text and deliberate clearing. Submitted replacement actions
also update the session replacement value. Replace has no direction controls in
the temporary common-dialog shell, so switching away from it retains the saved
Find direction. Native tests cover repeated switches, empty replacements, Unicode,
and unchanged document/history. Capturing unsent fields on dialog cancellation,
persistent search history and the source's unified dialog remain pending.

Cancel and window-close now capture live Find/Replace fields into the dialog's
retained buffers before native destruction sends FR_DIALOGTERM. The host accepts
the closing snapshot only for the matching dialog ID, saving query, replacement
and options for reopening in the current session. Native tests exercise both
WM_CLOSE and the Cancel command with unsent Unicode fields and verify that the
document/history is unchanged. Disk persistence and the unified source dialog
are still outstanding.

F3/Shift+F3 now import the current native caret and live dialog case/whole-word
options before repeating the saved query. As in NppCommands.cpp's getText2search
path, merely typing or clearing an unsubmitted Find field does not replace that
query for the editor's repeat command. Native tests cover changed options, an
externally moved caret, both directions, and unsubmitted/empty fields.

Case-insensitive literal matching now uses mappings generated directly from the
supplied CaseConvert.cxx symmetric ranges, singletons and complex fold strings.
Each source character's complete folded bytes must match; a search cannot consume
only part of an expanded character. Original UTF-8 offsets drive selection and
replacement. Tests cover sharp S, ligatures, final sigma, dotted I, micro sign,
partial-expansion rejection, case-sensitive exclusion and replace/undo. Regenerate
with scripts/generate-case-fold.ps1 and cargo fmt; generated data retains the
Scintilla license. Legacy-codepage folding remains outstanding.

An independent C++ oracle compiled with the supplied CaseConvert.cxx and
UniConversion.cxx now verifies the Rust fold result for every valid Unicode
scalar, including unchanged characters. It uses the capacity-taking converter
with spare output space, as the search path does. The oracle records changed
mappings in tests/fixtures/case_fold.tsv; ordinary Rust tests need no C++ compiler.
Regenerate with scripts/generate-case-fold-oracle.ps1 on MSVC 2022 Build Tools.
This validates folding mappings, not the entire Scintilla search engine.

Additional integration coverage verifies folded searches in both directions at
every character boundary across sharp S, ligatures and sigma, with wrap/case/word
options. Scoped replacement tests reject partial ranges and restore reverse
selections on undo. Native tests verify UTF-16 highlighting, Replace followed by
Find Next, and undo against original UTF-8 ranges when folded lengths differ.

Search > Wrap Around now exposes the session wrap setting. Native dialog requests
retain it because the common dialog has no wrap control. Count and Replace All
follow processAll scope rules: wrapping scans the document; without wrapping,
forward scope starts at selection minimum and backward scope ends at selection
maximum. In-selection overrides either. Native tests cover both directions,
reverse selections, scoped replacement, undo, and returning to whole-document
count. Persistent settings and the source dialog's own wrap checkbox remain pending.

After successful Replace All in Selection, the Windows command now explicitly
selects the adjusted scope forwards, matching processAll's SCI_SETSELECTION call.
Deleting the entire scope clears the in-selection toggle, following the source's
default nonzero autocheck threshold. A native test covers fold-expanded matches,
replacement growth, reverse input selection, complete removal, menu state and
undo restoration. The portable scoped-replacement API still maps its caller's
selection; this UI-specific reselection is applied by the Windows command.

Replace All now adjusts caret and selection endpoints using the supplied
Scintilla ReplaceTarget and SelectionRange::MoveForInsertDelete rules: delete
first, then insert, retaining direction and leaving collapsed carets at a match's
start. Whole-document replacement no longer jumps to EOF. Scoped replacement
uses the same endpoint mapping; the Windows in-selection command still explicitly
reselects the adjusted scope afterwards. Core tests cover endpoints inside and
outside matches, reversed selections, identical replacements, and undo/redo;
a native command test covers reverse selection and undo.

Search replacement transactions now retain identical text replacements, following
Scintilla's delete/insert history behavior. Undo restores the prior selection even
when bytes are unchanged; a new replacement clears redo and read-only documents
reject it without moving the selection. Tests cover all-identical and mixed-case
batches, no-match preservation of redo, and native Replace All undo/redo. General
editing operations still elide unchanged edits.

Text dirty-state tracking now uses history identities, matching the supplied
UndoHistory::IsSavePoint rule instead of comparing document contents. Undo/redo
can return to the saved revision; editing an abandoned history branch cannot
recreate that save point merely by producing equal bytes. Identical search
replacements mark the document dirty. Successful save records the current
revision; failed save and Save Copy retain the old save point; reload establishes
a new clean history. The saved text snapshot has been removed and dirty checks
are constant-time. Encoding comparison and deleted-file protection remain
independent. Tests cover these transitions in the core and native Replace All.
This does not yet implement Scintilla's full history API or save-point plugin
notifications, nor change existing metadata-only history semantics.

Single Replace now uses a search-target transaction, preserving undo and dirty
state even when replacement text is identical. It collapses the caret after the
replacement before finding the next match, as processReplace does. Its initial
search starts at selection minimum (forward) or maximum (backward), matching
FirstIncremental, so a larger selection first reveals a contained match instead
of skipping it. Native tests cover both directions, no-edit first search, Unicode
UTF-16 highlighting, identical replacement, undo/redo and save-point transitions.
Core tests cover identical/growing/empty replacements and invalid/read-only
atomicity. Regex behavior and the replace-stops-without-finding-next preference
remain outstanding.

Ordinary backward Find now starts just before selection maximum, matching
FindReplaceDlg::processFindNext and Scintilla's UTF-8 boundary adjustment. This
preserves overlapping matches (for example, both occurrences of aba in ababa)
that the former selection-minimum origin skipped. Saved, set-and-find, and
volatile searches share this origin. Replace retains FirstIncremental for both
its initial and subsequent searches. Portable tests cover reverse selections,
Unicode boundaries and wrap; a native test exercises Shift+F3, volatile previous,
and backward Replace. Regex-specific empty-match rules remain unimplemented.

Bookmarks now live in the portable Document and expose toggle, clear, and wrapped
next/previous navigation. Windows wires the source command IDs and Ctrl+F2, F2,
Shift+F2 defaults. Navigation collapses to the destination line start and works
read-only; bookmark changes do not add undo entries or make text dirty. The
status bar indicates a bookmarked current line while gutter rendering is pending.
Line insertion/deletion follows CellBuffer and LineMarkers rules, including
line-start insertion, deletion merging, whole-buffer clearing, and CRLF boundary
changes. Text undo/redo moves current bookmarks rather than restoring snapshots.
Tests cover these cases, batch edits, and native command navigation. Bookmarked
edits currently use a temporary text copy to compute sequential line changes;
large-document optimization, exhaustive source-oracle validation, reload/session
persistence, gutter symbols, inverse marks and marked-line operations remain work.

Bookmark movement is now checked against an independent C++ oracle linked to the
supplied CellBuffer, LineMarkers and supporting Scintilla sources. The fixture
covers 19,590 delete/insert cases across every input of up to four characters
from a/CR/LF, every range, replacements up to two characters, and each possible
initial marked line. Rust matches every recorded result. Regenerate with
scripts/generate-bookmarks-oracle.ps1 (MSVC 2022 Build Tools); normal Cargo tests
read the saved TSV and require no C++ compiler. This bounds the validation
scope; larger sequences, Unicode cases and complete marker APIs need more checks.
Inverse Bookmark now toggles every line, including a trailing empty line, using
the source command ID. Core/native tests cover read-only use, repeated inversion,
status updates and unchanged selection/history.

Copy/Cut Bookmarked Lines, Remove Bookmarked Lines and Remove Unmarked Lines are
now wired through source command IDs. Copy concatenates complete marked lines in
document order with original CRLF/LF/CR endings; it adds no terminator to a final
unterminated line. Deletion is one undo transaction, clears deleted bookmarks,
and handles an empty bookmarked final line without a text edit. Cut follows the
source's ordering: delete inside one undo group, then publish Unicode clipboard
text; clipboard failure leaves the deletion undoable. Editing commands are
read-only guarded while copy remains available. Core/native tests cover mixed
endings, Unicode, reverse selection undo, empty final lines, retained markers,
clipboard writer success/failure, and read-only behavior. Paste to bookmarked
lines, persistent bookmarks and the gutter remain pending.

Paste to Bookmarked Lines now replaces marked line bodies while preserving their
existing terminators, with a single undo entry and bottom-to-top selection
adjustment. Identical replacements retain history and dirty-state transitions;
empty final lines can receive text. Windows reads bounded CF_UNICODETEXT memory,
closes/unlocks it on normal/error paths, and performs no edit when text is absent
or reading fails. Tests use injected readers and a pure UTF-16 decoder, avoiding
changes to the user's clipboard; native tests verify the resulting editor text.
Coverage includes multiline and Unicode text, mixed EOLs, reversed selection,
identical/empty replacements, undo/redo, and read-only suppression. Unpaired
clipboard surrogates become replacement characters in the UTF-8 core. Bookmark
persistence, gutter rendering and complete Scintilla notification parity remain
outstanding.

A second bookmark oracle now executes the source's complete bottom-to-top paste
loop over 15,938 cases, covering all bookmark subsets for inputs up to four
characters from a/CR/LF and replacements up to two characters. It exposed a real
mismatch: deleting a line body between CR and LF temporarily joins the terminator
and moves its bookmark upwards, so the source later pastes into that earlier
line too. Paste now evaluates current markers and line positions sequentially.
History supports grouped sequential edits, preserving one user-visible undo step
without flattening interdependent replacements. All oracle cases match text and
markers; every case with history also checks undo, redo and saved-state recovery.
The targeted CRLF regression repeats undo/redo and restores reversed selection.
Regenerate both bookmark fixtures with scripts/generate-bookmarks-oracle.ps1.

The bookmark oracle now also checks 11,192 complete deletion/cut cases across
inputs up to five a/CR/LF characters and all initial bookmark subsets. This found
that cutting adjacent marked lines can join CR/LF before collecting an earlier
line, so the source's cut payload can differ from a precomputed copy. Removal now
uses current line positions/markers bottom-to-top in a grouped undo transaction;
Cut collects each actual removed line during that traversal. All cases match
source text, remaining markers and cut payload, with undo/redo and clean save
point checks. A focused regression covers CRLF joining and the changed clipboard
payload. Copy retains its separate, nonmutating behavior. These sequential paths
still need large-file performance work; the source oracle establishes correctness
for the stated bounded cases, not complete bookmark feature parity.

Single text edits and their undo/redo no longer clone a bookmarked document to
update markers. Marker updates read the original target bytes and cached line
number, deriving the post-deletion insertion boundary directly (including CR/LF
joining and splitting). All three source-oracle suites still match. A dedicated
allocation regression edits/undoes/redoes one byte in a 2 MiB bookmarked document
and checks that no allocation reaches 64 KiB, catching the former full-text copy.
Sequential marked-line commands benefit from this single-edit path. General
multi-edit batches still use a shadow text buffer, marker sets still rebuild on
line shifts, and long-line rescanning/large-file renderer work remain outstanding.

Search > Bookmark Matching Lines now applies the current normal/extended query,
case/whole-word options and the existing wrap/direction/selection scope rules.
It reads live Find/Replace dialog fields using the same path as Count. Complete,
nonoverlapping matches add bookmarks to every touched line; a match ending at a
line start excludes that following line, matching ProcessMarkAll's targetEnd-1
rule. Existing bookmarks remain, and selection, read-only state and text history
are unchanged. Tests cover multiline mixed-EOL matches, folded Unicode matches,
partial scopes, errors, reverse selection, wrapping and native read-only use.
This explicitly named bookmark command exposes the source's additive bookmark
operation; the full Mark dialog, match indicators, purge setting, regex zero-length
matches and persistent marking preferences remain outstanding.

Reload bookmark behavior was checked through Notepad_plus::doReload,
FileManager::reloadBuffer and loadFileData's SCI_CLEARALL: successful reload
clears bookmarks. Regression tests now distinguish that from failed decode,
which retains the Rust document and its markers. Clear Bookmarks in Selection
is also available in Search. It follows FindReplaceDlg::clearMarks from the
selection's first line through the line containing end-1, preserving bookmarks
outside that range. This includes the source's empty-selection behavior: inside
a line clears that line; at a later line start clears none; at document start
clears the first line. Native/core tests cover reverse selection, mixed EOLs,
read-only use and unchanged history. Session persistence and the full Mark UI
remain separate outstanding work.

The portable core now reads/writes the source-shaped NotepadPlus/Session XML
format using quick-xml 0.38 (compatible with the project's Rust 1.85 minimum).
It represents both views, active indices, ordered file entries, bookmarks and
folds. File attributes are retained verbatim as decoded strings, including
position, language, encoding, read-only, backup, timestamp, document-map, pin and
future attributes; FileBrowser/other session subtrees and extra file nodes are
retained. UTF-8/escaped Unicode paths round-trip. Invalid marker indices are
ignored and invalid active indices default as in Parameters.cpp. Tests cover
source-shaped files, round trips, missing views, XML escaping and malformed
input. Backup restoration, automatic
startup/shutdown persistence and split-view UI remain to be connected. Duplicate
view/session nodes, missing filenames and unsupported XML content are currently
rejected, rather than applying the source reader's permissive recovery rules.

Windows File > Save Session now captures named, existing files in tab order and
writes session XML through the atomic file writer. Untitled and missing files
are omitted, while the original active tab index is retained as in the source.
Capture includes selection direction, bookmarks, legacy encoding code pages,
user read-only state, and the active editor's first visible line. Unicode
selection positions use UTF-8 byte offsets; legacy documents use encoded byte
offsets. Saving a session leaves document contents, dirty state and undo history
unchanged. Core and native tests cover Unicode/Shift-JIS positions, tab order,
bookmarks, dirty documents and write failures. Horizontal scroll,
language/fold state and split-view capture remain incomplete;
manual session saving does not create unsaved-content backups.

Session file entries can now open their named files and restore core editor
state. Supported explicit code pages are Windows-1252, Windows-1251 and Shift-JIS;
unspecified encoding uses existing Unicode detection. Unsupported code pages
fail explicitly. Restoration converts legacy byte positions to the core's UTF-8
positions, retains anchor/caret direction, adds valid bookmarks without clearing
existing ones and restores user read-only state without changing text/history.
Stale offsets are clamped; offsets inside multibyte characters round down to a
character boundary to satisfy the Rust core's selection invariant. Tests cover
Unicode BOM variants, all supported legacy encodings, existing dirty buffers,
duplicate/out-of-range marks, invalid offsets and failed position mapping.
This is the reusable document restoration layer used by Windows Load Session.

Windows File > Load Session parses the complete XML before changing buffers,
opens files in session order, reuses existing tabs without discarding dirty
contents, and continues loading after individual file failures with an aggregate
error report. It resolves the active index after failed entries are removed, as
the source does, and restores the selected file's first visible line. Initial
empty tabs are replaced by the existing open-file path; empty sessions are a
no-op. Native tests cover partial failures, repeated loads, dirty/read-only
buffers, legacy encoding positions, malformed XML, active-view selection and
scrolling. Both session view lists currently share the one Windows tab strip;
independent split-view buffers/positions,
snapshot backups, missing-file placeholders, session-extension preferences and
multi-instance session launching remain incomplete.

The Windows adapter now retains the first visible line for each buffer when
switching tabs and restores it on return. Session saving includes inactive tab
positions, and loading applies each file's position before moving to the next.
Scroll positions are clamped to the document's current line count; closing a
buffer removes its cached position. A native regression test exercises two
different scroll positions through previous/next tab commands and a session
save/load round trip, also checking unchanged dirty and undo state. Horizontal
scroll and independent positions for cloned split views are still pending.

Plugin session messages now include GETNBSESSIONFILES (with the source's 32-bit
validity BOOL), GETSESSIONFILES (main then sub view order, including duplicate
entries), SAVESESSION and SAVECURRENTSESSION. Save calls use the atomic writer,
omit missing explicit files and return the caller's filename pointer on success.
Current-session saves use the published document state and cached scroll lines;
plugins invoking them during an unprocessed native edit can observe the prior
snapshot. LOADSESSION copies the caller's filename into the UI event queue and
uses the same manual load path; a null filename opens the session dialog. This
load is deferred until the callback returns, unlike the source's synchronous
load, so immediate follow-up plugin queries can still see the earlier workspace.
The session XML reader's documented strictness and split-view limits also apply.

The native menu now follows the supplied Notepad_plus.rc ordering and nested
groups for supported commands. Edit includes Insert, Copy to Clipboard, Indent,
Convert Case, Line Operations, EOL Conversion, Blank Operations and Read-Only;
File groups containing-folder and multiple-document actions, Search groups
bookmarks, and View groups Zoom and Tab commands. The separate Line Endings
top-level menu was removed in favor of Edit > EOL Conversion. All 149 previously
exposed commands remain available with their existing IDs and shortcuts.
Unimplemented source commands and their empty groups are omitted. Rewrite-only
encoding, search and interface controls are retained in related submenus.

Find in Files now has a dedicated modeless dialog (Ctrl+Shift+F), folder browser,
current-document-directory action, editable query/filter/directory history,
match-case and whole-word controls, recursion and hidden-folder switches, and
Normal, Extended and regular-expression modes. The portable directory walker
accepts whitespace-separated include patterns, !filename exclusions, first-level
!\directory exclusions and recursive !+\directory exclusions; exclusion-only
filters imply *.*. Reparse/symlink directories are skipped to avoid traversal
loops. Unicode BOMs are detected and other invalid UTF-8 falls back to Windows-
1252, so system-specific ANSI detection remains incomplete. Open named buffers
are searched from a snapshot of their current text, including dirty contents.

Search runs off the UI thread with a bounded results channel and Stop support.
Literal searches poll cancellation in 64 KiB candidate chunks and report
non-overlapping matches; regex searches poll between matches and use a bounded
backtracking engine. The docked results list groups matches by file, shows line
numbers and previews, retains prior searches, opens exact match selections on
double-click/Enter and supports F4/Shift+F4 navigation. Its context menu provides
copy selected line/all results, clear, stop and hide actions. Runtime checks
against the compiled application exercised directory/file exclusions, recursion,
Unicode filenames, case-insensitive search, opening/selection, regex lookbehind,
non-overlapping matches, dropdown history and retained results. No new unit tests
were added for this implementation.

Remaining Find in Files parity includes the unified tabbed Find/Replace dialog,
Boost-specific regex syntax, project-panel search, persisted history/preferences,
collapsible/styled result text with inline hit highlighting, newest-first result
blocks and live adjustment of result positions after edits. Results currently
use a native list, append new searches and truncate previews to 500 characters.
Large files are read into memory; cancellation cannot interrupt a filesystem
read or a running individual regex operation. Replace in Files is separate and
has not been implemented by this change.
