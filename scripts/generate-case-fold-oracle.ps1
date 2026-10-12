$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    # MSVC 2022 Build Tools, matching the existing native source fixtures.
    & cmd /c '"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul && cl /nologo /EHsc /std:c++17 /O2 crates\editor-core\tests\fixtures\case_fold.cpp notepad_plus_plus\scintilla\src\CaseConvert.cxx notepad_plus_plus\scintilla\src\UniConversion.cxx /Fotarget/ /Fe:target\case_fold_fixture.exe'
    if ($LASTEXITCODE -ne 0) { throw 'Oracle compilation failed' }
    & .\target\case_fold_fixture.exe crates/editor-core/tests/fixtures/case_fold.tsv
    if ($LASTEXITCODE -ne 0) { throw 'Oracle generation failed' }
} finally { Pop-Location }
