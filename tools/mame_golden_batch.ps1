# Génère les goldens MAME (trames 120 et 600) pour une liste de jeux.
# Exemple : powershell tools/mame_golden_batch.ps1 -Games kof94,nam1975
# Aucune ROM n'est copiée ; les images vont dans tests-data/golden/<jeu> (local, hors git).
param(
  [Parameter(Mandatory=$true)][string[]]$Games,
  [string]$Out = "tests-data/golden",
  [int[]]$Frames = @(120,600)
)
$ErrorActionPreference = "Continue"
$Games = $Games | ForEach-Object { $_ -split ',' } | Where-Object { $_ -ne "" }
foreach ($g in $Games) {
  Write-Host "== $g"
  & "$PSScriptRoot/mame_golden.ps1" -Game $g -Frames $Frames -Out (Join-Path $Out $g)
}
