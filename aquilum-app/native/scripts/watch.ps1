# Rebuilds and restarts the native window whenever the code changes.
#
# Watches the window and core sources, style tokens and translations. After a save it builds
# the native-watch profile (release-level optimization but no LTO, so rebuilds take seconds) and restarts the window:
# the old one closes normally, so it flushes unsaved edits and remembers its size, position and
# open note. If the build fails, the old window keeps running.
#
# Usage: npm run dev (from aquilum-app) or native\watch.bat [window arguments]; with no arguments it opens the last used vault.

param([Parameter(ValueFromRemainingArguments = $true)][string[]]$AppArgs)

$ErrorActionPreference = 'Stop'
$native = Split-Path $PSScriptRoot -Parent
$app = Split-Path $native -Parent
$exe = Join-Path (Split-Path $app -Parent) '.artifacts\cargo-native\native-watch\aquilum-native.exe'

if (-not $AppArgs) {
    $AppArgs = @()
}
$AppArgs += '--keep-position'

# The profile is defined with flags so the working Cargo.toml stays untouched.
$cargoArgs = @(
    'build', '-p', 'aquilum-native', '--profile', 'native-watch',
    '--config', "profile.native-watch.inherits='dev'",
    '--config', "profile.native-watch.package.'*'.opt-level=3",
    '--config', 'profile.native-watch.opt-level=3',
    '--config', 'profile.native-watch.debug-assertions=false',
    '--config', "profile.native-watch.debug='line-tables-only'",
    '--config', 'profile.native-watch.incremental=false'
)

$watched = @(
    (Join-Path $native 'src'),
    (Join-Path $native 'build.rs'),
    (Join-Path $native 'Cargo.toml'),
    (Join-Path $app 'core\src'),
    (Join-Path $native 'assets')
)

function Get-Stamp {
    $latest = [datetime]::MinValue
    foreach ($path in $watched) {
        if (-not (Test-Path $path)) { continue }
        foreach ($file in Get-ChildItem $path -Recurse -File -ErrorAction SilentlyContinue) {
            if ($file.LastWriteTimeUtc -gt $latest) { $latest = $file.LastWriteTimeUtc }
        }
    }
    $latest
}

function Quote([string]$arg) {
    if ($arg -match '[\s"]') { '"' + ($arg -replace '"', '\"') + '"' } else { $arg }
}

function Stop-App($process) {
    if (-not $process -or $process.HasExited) { return }
    # Normal close (WM_CLOSE): the window saves the edit, its size and the open note.
    [void]$process.CloseMainWindow()
    if (-not $process.WaitForExit(5000)) { $process.Kill() }
}

function Build {
    Write-Host ''
    Write-Host ("[{0:HH:mm:ss}] building..." -f (Get-Date)) -ForegroundColor Cyan
    Push-Location $app
    try { & cargo @cargoArgs } finally { Pop-Location }
    $LASTEXITCODE -eq 0
}

Get-Process aquilum-app, aquilum-native -ErrorAction SilentlyContinue | ForEach-Object { Stop-App $_ }
$process = $null
$stamp = Get-Stamp
try {
    while ($true) {
        if (Build) {
            Stop-App $process
            $process = Start-Process $exe -ArgumentList (($AppArgs | ForEach-Object { Quote $_ }) -join ' ') -PassThru
            Write-Host ("[{0:HH:mm:ss}] window started, waiting for changes (Ctrl+C to exit)" -f (Get-Date)) -ForegroundColor Green
        } else {
            Write-Host 'Build failed: the previous window keeps running, waiting for a fix.' -ForegroundColor Yellow
        }
        # Wait for a change, then for half a second of quiet: the editor may save several files.
        do {
            Start-Sleep -Milliseconds 500
            $next = Get-Stamp
        } while ($next -eq $stamp)
        do {
            $stamp = $next
            Start-Sleep -Milliseconds 500
            $next = Get-Stamp
        } while ($next -ne $stamp)
    }
} finally {
    Stop-App $process
}
