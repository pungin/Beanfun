<#
.SYNOPSIS
    Read the client-integrity values beanfun's TW OTP endpoint checks.

.DESCRIPTION
    beanfun asks the caller to state which Gamania Games Manager build is
    asking: a file version and the SHA-256 of GGMWebStart.dll. This reads
    both off an installed GGM and prints the document the app publishes.

    Run it on a Windows machine with the *new* GGM installed. The version
    comes from the file's version resource, which only Windows can read —
    which is why a CI runner can produce the hash but never the version.

.PARAMETER Dll
    Read a specific GGMWebStart.dll instead of locating one.

.PARAMETER Write
    Write ggm-client.json at the repository root, UTF-8 without BOM.
    A BOM makes the document unparseable, which silently drops every user
    to the compiled-in pair — the fix looks published and helps nobody.

.EXAMPLE
    .\scripts\ggm-client.ps1
    .\scripts\ggm-client.ps1 -Write
    .\scripts\ggm-client.ps1 -Dll 'D:\GGM\GGMWebStart.dll' -Write
#>
[CmdletBinding()]
param(
    [string]$Dll,
    [switch]$Write
)

$ErrorActionPreference = 'Stop'
$InstallerUrl = 'https://tw.beanfun.com/ggm/index.aspx'

function Find-GgmDll {
    if ($Dll) {
        if (-not (Test-Path -LiteralPath $Dll)) { throw "no such file: $Dll" }
        return (Resolve-Path -LiteralPath $Dll).Path
    }

    $candidates = @()

    # The documented install path.
    $key = 'HKLM:\SOFTWARE\gamaniaGamesManager'
    if (Test-Path $key) {
        $dir = (Get-ItemProperty -Path $key -ErrorAction SilentlyContinue).InstallPath
        if ($dir) { $candidates += (Join-Path $dir 'GGMWebStart.dll') }
    }

    # The protocol handler, for installs that key differently.
    $cmdKey = 'Registry::HKEY_CLASSES_ROOT\gamaniagames\shell\open\command'
    if (Test-Path $cmdKey) {
        $command = (Get-ItemProperty -Path $cmdKey -ErrorAction SilentlyContinue).'(default)'
        if ($command) {
            $exe = if ($command.Trim().StartsWith('"')) { $command.Split('"')[1] }
                   else { $command.Split(' ')[0] }
            if ($exe) {
                $dir = Split-Path -Parent $exe
                if ($dir) { $candidates += (Join-Path $dir 'GGMWebStart.dll') }
            }
        }
    }

    foreach ($path in $candidates) {
        if (Test-Path -LiteralPath $path) { return (Resolve-Path -LiteralPath $path).Path }
    }

    throw "no GGMWebStart.dll found. Install the Game Manager from $InstallerUrl, or pass -Dll <path>."
}

$path = Find-GgmDll
$cv   = (Get-Item -LiteralPath $path).VersionInfo.FileVersion
$hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
$arch = if ([Environment]::Is64BitOperatingSystem) { 'x64' } else { 'x86' }

if (-not $cv)   { throw "no FileVersion on $path" }
if ($hash.Length -ne 64) { throw "hash is not a SHA-256: $hash" }
# The app validates both and treats anything else as if the file were
# absent, so catch it here rather than after publishing.
if ($cv -notmatch '^[0-9.]+$') { throw "version must be digits and dots: $cv" }

$document = [ordered]@{
    cv        = $cv
    hash      = $hash
    arch      = $arch
    installer = $InstallerUrl
    note      = "Client-integrity values beanfun's TW OTP endpoint checks. Edit and push to code to hotfix every user without a release; see docs/GGM-CLIENT-HOTFIX.md. Save as UTF-8; hash must be exactly 64 hex characters."
}

Write-Host "dll  : $path"
Write-Host "cv   : $cv"
Write-Host "hash : $hash"
Write-Host "arch : $arch"
Write-Host ''

$json = ($document | ConvertTo-Json -Depth 3)
Write-Host $json

if ($Write) {
    $root = Split-Path -Parent $PSScriptRoot
    $out  = Join-Path $root 'ggm-client.json'
    # UTF8Encoding($false) = no BOM. Set-Content -Encoding utf8 writes one
    # on Windows PowerShell, and a BOM breaks the document.
    [System.IO.File]::WriteAllText($out, $json + "`n", [System.Text.UTF8Encoding]::new($false))
    Write-Host ''
    Write-Host "wrote $out (UTF-8, no BOM)"
    Write-Host 'Next: commit and push to `code`, then verify the raw URL returns 200.'
}
