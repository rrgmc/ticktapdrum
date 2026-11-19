# Builds TickTapDrum for Windows and packages it into dist/:
#   ticktapdrum-<version>-windows-x64-setup.exe  (installer, per user, no admin rights)
#   ticktapdrum-<version>-windows-x64.zip        (the program alone)
# Usage, from the repository root:  pwsh apps/dm-desktop/packaging/windows/package.ps1 0.2.0
param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'

cargo build --release --locked -p dm-desktop
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$dist = Join-Path (Get-Location) 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$exe = Join-Path (Get-Location) 'target\release\ticktapdrum.exe'

# The zip: the program under its display name.
$stage = Join-Path ([IO.Path]::GetTempPath()) "ticktapdrum-$Version"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item $exe (Join-Path $stage 'TickTapDrum.exe')
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath (Join-Path $dist "ticktapdrum-$Version-windows-x64.zip") -Force
Remove-Item -Recurse -Force $stage

# The installer, with Inno Setup's compiler.
$iscc = (Get-Command iscc -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    ) |
        Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw 'Inno Setup 6 (ISCC.exe) is not installed' }
& $iscc "/DAppVersion=$Version" "/DSourceExe=$exe" "/DOutputDir=$dist" (Join-Path $PSScriptRoot 'ticktapdrum.iss')
if ($LASTEXITCODE -ne 0) { throw 'iscc failed' }
Get-ChildItem $dist
