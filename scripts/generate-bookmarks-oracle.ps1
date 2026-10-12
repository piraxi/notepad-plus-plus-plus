$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    New-Item -ItemType Directory -Force target | Out-Null
    & cmd /c '"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul && cl /nologo /EHsc /std:c++17 /O2 /DNDEBUG /I notepad_plus_plus\scintilla\include /I notepad_plus_plus\scintilla\src crates\editor-core\tests\fixtures\bookmarks.cpp notepad_plus_plus\scintilla\src\CellBuffer.cxx notepad_plus_plus\scintilla\src\PerLine.cxx notepad_plus_plus\scintilla\src\UndoHistory.cxx notepad_plus_plus\scintilla\src\ChangeHistory.cxx notepad_plus_plus\scintilla\src\UniConversion.cxx notepad_plus_plus\scintilla\src\RunStyles.cxx /Fotarget/ /Fe:target\bookmarks_fixture.exe'
    if ($LASTEXITCODE -ne 0) { throw 'Oracle compilation failed' }
    & .\target\bookmarks_fixture.exe crates/editor-core/tests/fixtures/bookmarks.tsv crates/editor-core/tests/fixtures/bookmark_paste.tsv crates/editor-core/tests/fixtures/bookmark_delete.tsv
    if ($LASTEXITCODE -ne 0) { throw 'Oracle generation failed' }
} finally { Pop-Location }

