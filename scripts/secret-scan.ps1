# Secret-scan: refuses to ship if any file that git would track contains
# patterns that look like an accidentally-committed secret.
#
# Scope: this scan deliberately ignores anything covered by .gitignore.
# Local demo keys, evidence bundles, and other generated artifacts live in
# gitignored paths and MUST NOT trip the scan. The scan exists to catch
# real commit-time leaks, not to police a developer's local workspace.
#
# Test files are excluded: they contain negative assertions referencing
# these patterns to prove their absence elsewhere.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -Path $root

$patterns = @(
  'BEGIN PRIVATE KEY',
  '"admin_token"\s*:',
  '"bearer"\s*:',
  '"operator_private"\s*:',
  '"private_key"\s*:',
  '"secret_seed_b64"\s*:',
  '"secret_key"\s*:',
  'IPPAN_ADMIN_TOKEN\s*='
)

$haveGit = ($null -ne (Get-Command git -ErrorAction SilentlyContinue)) -and (Test-Path (Join-Path $root '.git'))

if ($haveGit) {
  $files = & git ls-files --cached --others --exclude-standard |
    Where-Object { $_ -notmatch '^(tests/|docs/|scripts/|\.git/)' } |
    Where-Object { $_ -notmatch '\.md$' }
} else {
  $files = Get-ChildItem -Path examples,src,Cargo.toml -Recurse -File -ErrorAction SilentlyContinue |
    ForEach-Object { $_.FullName.Substring($root.Length + 1) }
}

$failed = $false
foreach ($f in $files) {
  if (-not (Test-Path $f)) { continue }
  foreach ($pat in $patterns) {
    $hits = Select-String -Path $f -Pattern $pat -ErrorAction SilentlyContinue
    if ($hits) {
      foreach ($h in $hits) {
        Write-Host ("FAIL: {0}:{1}: {2}" -f $h.Path, $h.LineNumber, $h.Line)
      }
      $failed = $true
    }
  }
}

if ($failed) {
  Write-Host 'secret-scan: FAILED - refusing to ship'
  exit 1
}
Write-Host 'secret-scan: clean'
