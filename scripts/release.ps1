<#
.SYNOPSIS
  Builds, signs and publishes a Windows release of Apus, including the latest.json the app
  reads to find updates.

.DESCRIPTION
  Run it from anywhere, on the main branch, after the version in aquilum-app/core/Cargo.toml
  and aquilum-app/native/Cargo.toml has been raised, committed and pushed.

    .\scripts\release.ps1                       # asks for the signing password
    .\scripts\release.ps1 -Notes "What changed" # release description
    .\scripts\release.ps1 -DryRun               # builds and signs, publishes nothing

  The updater key is created once with:
    cargo packager signer generate --path "$HOME\.apus\updater.key"
  Its public half is the PUBLIC_KEY constant in aquilum-app/native/src/updater/release.rs.
  Keep the private key and its password safe: without them nobody can publish updates the
  installed apps will accept.

.NOTES
  Needs: Rust + cargo-packager, MSVC Build Tools, git, and the GitHub CLI (gh) logged in.
#>
param(
    [string]$Notes = "",
    [string]$KeyPath = (Join-Path $HOME ".apus\updater.key"),
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$Repo = "sconelli24/Aquilum"

function Step($text) { Write-Host "`n==> $text" -ForegroundColor Cyan }
function Fail($text) { Write-Host "ERROR: $text" -ForegroundColor Red; exit 1 }
function Run($file, [string[]]$arguments) {
    & $file @arguments
    if ($LASTEXITCODE -ne 0) { Fail "'$file $($arguments -join ' ')' failed with exit code $LASTEXITCODE" }
}

$root = Split-Path -Parent $PSScriptRoot
$app = Join-Path $root "aquilum-app"
$out = Join-Path $root ".artifacts\packages"
$env:Path += ";$HOME\.cargo\bin;C:\Program Files\GitHub CLI"

# ---- checks --------------------------------------------------------------------------------
Step "Checking the environment"
if (-not (Test-Path $KeyPath)) { Fail "Signing key not found at $KeyPath. Create it with: cargo packager signer generate --path `"$KeyPath`"" }
foreach ($tool in "cargo", "git", "gh") {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Fail "'$tool' is not installed or not on PATH" }
}
if (-not $DryRun) {
    & gh auth status *> $null
    if ($LASTEXITCODE -ne 0) { Fail "gh is not logged in. Run: gh auth login" }
}

Set-Location $root
$branch = (git branch --show-current).Trim()
if ($branch -ne "main") { Fail "You are on '$branch'. Switch to main first." }
$dirty = git status --porcelain --untracked-files=no
if ($dirty) { Fail "There are uncommitted changes:`n$dirty" }
Run git @("fetch", "origin", "--tags", "--quiet")
if ((git rev-parse HEAD).Trim() -ne (git rev-parse origin/main).Trim()) { Fail "main differs from origin/main. Push (or pull) first so the release matches the published source." }

function VersionOf($file) {
    $match = Select-String -Path $file -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if (-not $match) { Fail "No version found in $file" }
    return $match.Matches[0].Groups[1].Value
}
$version = VersionOf (Join-Path $app "native\Cargo.toml")
if ($version -ne (VersionOf (Join-Path $app "core\Cargo.toml"))) { Fail "core and native have different versions" }
$tag = "v$version"
if (git tag --list $tag) { Fail "Tag $tag already exists. Raise the version in Cargo.toml (core and native) first." }
Write-Host "Releasing Apus $version ($tag)"

# ---- build ---------------------------------------------------------------------------------
Step "Building the release binary"
Set-Location $app
Run cargo @("build", "--release", "-p", "aquilum-native", "--features", "installed")
$bin = Join-Path $root ".artifacts\cargo-native\release"
Copy-Item (Join-Path $bin "aquilum-native.exe") (Join-Path $bin "aquilum-app.exe") -Force

# ---- sign and package ----------------------------------------------------------------------
Step "Packaging and signing the installer"
$secure = Read-Host "Password of the signing key" -AsSecureString
$password = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
$env:CARGO_PACKAGER_SIGN_PRIVATE_KEY = (Get-Content $KeyPath -Raw)
$env:CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD = $password
if (Test-Path $out) { Get-ChildItem $out -Filter "*_${version}_*" | Remove-Item -Force }
try {
    Run cargo @("packager", "-p", "aquilum-native", "--release", "--formats", "nsis", "--out-dir", $out)
} finally {
    Remove-Item Env:\CARGO_PACKAGER_SIGN_PRIVATE_KEY, Env:\CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}

$built = Get-ChildItem $out -Filter "*_${version}_x64-setup.exe" | Select-Object -First 1
if (-not $built) { Fail "Installer for $version not found in $out" }
$builtSig = "$($built.FullName).sig"
if (-not (Test-Path $builtSig)) { Fail "No signature next to the installer. Wrong password or key?" }

# ---- publish files -------------------------------------------------------------------------
Step "Preparing the release files"
$stage = Join-Path $root ".artifacts\release\$tag"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory $stage | Out-Null
$name = "Apus_${version}_x64-setup.exe"
Copy-Item $built.FullName (Join-Path $stage $name)
$signature = (Get-Content $builtSig -Raw).Trim()
$entry = [ordered]@{ signature = $signature; url = "https://github.com/$Repo/releases/download/$tag/$name" }
$manifest = [ordered]@{
    version  = $version
    notes    = $Notes
    pub_date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.000Z")
    platforms = [ordered]@{ "windows-x86_64" = $entry; "windows-x86_64-nsis" = $entry }
}
$latest = Join-Path $stage "latest.json"
[IO.File]::WriteAllText($latest, ($manifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))
Write-Host "Installer : $(Join-Path $stage $name)"
Write-Host "Manifest  : $latest"

if ($DryRun) { Write-Host "`nDry run: nothing was tagged or published." -ForegroundColor Yellow; exit 0 }

# ---- publish -------------------------------------------------------------------------------
Step "Tagging and publishing $tag"
Set-Location $root
Run git @("tag", "-a", $tag, "-m", "Apus $version")
Run git @("push", "origin", $tag)
$body = if ($Notes) { $Notes } else { "Apus $version. Windows installer (unsigned by Microsoft: SmartScreen may warn; choose *More info -> Run anyway*). Source code: this repository at tag ``$tag``. Installed copies of Apus offer this update on startup." }
Run gh @("release", "create", $tag, (Join-Path $stage $name), $latest, "--repo", $Repo, "--title", "Apus $version", "--notes", $body)

Write-Host "`nPublished: https://github.com/$Repo/releases/tag/$tag" -ForegroundColor Green
Write-Host "Apps with an older Apus (0.4.1 or later) will find it on their next start."
