# Génère les captures de référence (golden frames) avec MAME.
# Exemple : powershell tools/mame_golden.ps1 -Game mslug -Frames 120,600,1500 -Out tests-data/golden/mslug
param(
  [Parameter(Mandatory=$true)][string]$Game,
  [Parameter(Mandatory=$true)][int[]]$Frames,
  [Parameter(Mandatory=$true)][string]$Out,
  [string]$Mame = "X:\Mame_0287\mame.exe",
  [string]$RomPath = "",          # dossier des ROMs ; par défaut : celui de neogeo.toml (paths.roms_dir)
  [string]$Bios = "",             # ex. "euro" (Europe MVS v2) ; vide = BIOS par défaut de MAME
  [switch]$Show                   # retire -video none si les captures sortent vides
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Mame)) { throw "MAME introuvable : $Mame" }
if ($RomPath -eq "" -and (Test-Path "neogeo.toml")) {
  $m = Select-String -Path "neogeo.toml" -Pattern '^\s*roms_dir\s*=\s*"(.*)"' | Select-Object -First 1
  if ($m) { $RomPath = $m.Matches[0].Groups[1].Value }
}
if ($RomPath -eq "") { throw "Indiquer -RomPath ou renseigner paths.roms_dir dans neogeo.toml" }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$outAbs = (Resolve-Path $Out).Path
$env:NG_FRAMES = ($Frames -join ",")
$env:NG_PREFIX = "f"
$lua = (Resolve-Path "tools/mame_snap.lua").Path
$args = @($Game, "-rompath", $RomPath, "-sound", "none", "-nothrottle", "-skip_gameinfo",
          "-snapshot_directory", $outAbs, "-snapname", "%i", "-autoboot_script", $lua)
if (-not $Show) { $args += @("-video", "none") }
if ($Bios -ne "") { $args += @("-bios", $Bios) }
& $Mame @args
# MAME range les captures dans <snapshot_directory>/<jeu>/ : les remonter.
Get-ChildItem -Path (Join-Path $outAbs $Game) -Filter "*.png" -ErrorAction SilentlyContinue |
  ForEach-Object { Move-Item -Force $_.FullName (Join-Path $outAbs $_.Name) }
Get-ChildItem $outAbs -Filter "f*.png" | Select-Object Name, Length
