$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$sourceText = [IO.File]::ReadAllText((Join-Path $repoRoot 'notepad_plus_plus/scintilla/src/CaseConvert.cxx'))
$mapping = [Collections.Generic.SortedDictionary[int,byte[]]]::new()
foreach ($name in @('symmetricCaseConversionRanges', 'symmetricCaseConversions')) {
    $body = [regex]::Match($sourceText, ($name + '\[\] = \{([\s\S]*?)\};')).Groups[1].Value
    $numbers = @([regex]::Matches($body, '(?m)^[0-9,]+') | ForEach-Object { $_.Value.TrimEnd(',').Split(',') | ForEach-Object { [int]$_ } })
    $stride = if ($name.EndsWith('Ranges')) { 4 } else { 2 }
    for ($i = 0; $i -lt $numbers.Count; $i += $stride) {
        $length = if ($stride -eq 4) { $numbers[$i + 2] } else { 1 }
        $pitch = if ($stride -eq 4) { $numbers[$i + 3] } else { 1 }
        for ($j = 0; $j -lt $length; $j++) {
            $mapping[$numbers[$i + 1] + $j * $pitch] = [Text.Encoding]::UTF8.GetBytes([char]::ConvertFromUtf32($numbers[$i] + $j * $pitch))
        }
    }
}
function Convert-SourceBytes([string]$value) {
    $bytes = [Collections.Generic.List[byte]]::new()
    for ($i = 0; $i -lt $value.Length; $i++) {
        if ($value[$i] -eq '\') {
            if ($value[$i + 1] -ne 'x') { throw 'Unexpected source escape' }
            $bytes.Add([Convert]::ToByte($value.Substring($i + 2, 2), 16)); $i += 3
        } else { $bytes.Add([byte][char]$value[$i]) }
    }
    return ,$bytes.ToArray()
}
$complex = [regex]::Match($sourceText, 'complexCaseConversions =([\s\S]*?);').Groups[1].Value
foreach ($entry in [regex]::Matches($complex, '"([^"\r\n]*)"')) {
    $fields = $entry.Groups[1].Value.Split('|')
    if ($fields[1].Length -ne 0) {
        $origin = [Text.Encoding]::UTF8.GetString((Convert-SourceBytes $fields[0]))
        $mapping[[char]::ConvertToUtf32($origin, 0)] = Convert-SourceBytes $fields[1]
    }
}
if ($mapping.Count -lt 1000) { throw 'Unexpected folding table' }
$lines = [Collections.Generic.List[string]]::new()
$lines.Add('// Generated from supplied Scintilla CaseConvert.cxx by scripts/generate-case-fold.ps1.')
$lines.Add('// Copyright 2013 by Neil Hodgson <neilh@scintilla.org>.')
foreach ($line in (Get-Content -LiteralPath (Join-Path $repoRoot 'notepad_plus_plus/scintilla/License.txt'))) { $lines.Add('// ' + $line) }
$lines.Add('pub(super) const FOLDS: &[(u32, &[u8])] = &[')
foreach ($entry in $mapping.GetEnumerator()) {
    $bytes = ($entry.Value | ForEach-Object { '\x{0:x2}' -f $_ }) -join ''
    $lines.Add('    (' + $entry.Key + ', b"' + $bytes + '"),')
}
$lines.Add('];')
[IO.File]::WriteAllText((Join-Path $repoRoot 'crates/editor-core/src/case_fold_data.rs'), ($lines -join "`n") + "`n")
