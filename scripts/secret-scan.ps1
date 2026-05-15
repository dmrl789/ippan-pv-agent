# Tiny secret-scan covering committed evidence, config, and code fixtures.
# Documentation files are excluded - they may discuss these terms.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$patterns = @(
  'BEGIN PRIVATE KEY',
  '"admin_token"\s*:',
  '"bearer"\s*:',
  '"operator_private"\s*:',
  '"private_key"\s*:',
  'IPPAN_ADMIN_TOKEN\s*='
)

# Generated evidence, committed config, and code fixtures.
# Tests are excluded because they contain negative assertions that legitimately
# reference these tokens to prove their absence elsewhere.
$paths = @(
  (Join-Path $root 'examples'),
  (Join-Path $root 'data')
)

$failed = $false
foreach ($p in $paths) {
  if (-not (Test-Path $p)) { continue }
  foreach ($pat in $patterns) {
    $hits = Get-ChildItem -Path $p -Recurse -File -ErrorAction SilentlyContinue |
      Select-String -Pattern $pat -SimpleMatch:$false
    if ($hits) {
      foreach ($h in $hits) {
        Write-Host ("FAIL: {0}:{1}: {2}" -f $h.Path, $h.LineNumber, $h.Line)
      }
      $failed = $true
    }
  }
}

if ($failed) {
  exit 1
}
Write-Host 'secret-scan: clean'
