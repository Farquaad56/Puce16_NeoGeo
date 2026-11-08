# Importe tasks/board.json dans Hermes Kanban (board "neogeo", profil rust-dev).
# Chaîne linéaire : chaque carte a pour parent la précédente. Idempotent (clé puce16ng-<id>).
param([string]$Root = "H:\__Emulator__\Puce16_NeoGeo", [string]$Assignee = "rust-dev", [string]$BoardSlug = "neogeo")
$ErrorActionPreference = "Continue"
& hermes kanban boards create $BoardSlug --name "Neo Geo" 2>&1 | Out-Null
& hermes kanban boards switch $BoardSlug
$cards = Get-Content "$Root\tasks\board.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$ids = @{}
foreach ($t in $cards) {
  $a = @("kanban","create",$t.title,"--body-file","$Root\$($t.file)","--idempotency-key","puce16ng-$($t.id)","--workspace","dir:$Root","--tenant",$BoardSlug,"--assignee",$Assignee,"--json")
  foreach ($d in $t.deps) { if ($ids.ContainsKey($d)) { $a += @("--parent",$ids[$d]) } }
  $raw = & hermes @a
  try { $out = $raw | ConvertFrom-Json } catch { Write-Host "ERREUR $($t.id) : $raw"; exit 1 }
  $id = if ($out.task_id) { $out.task_id } else { $out.id }
  if (-not $id) { Write-Host "ERREUR $($t.id) : pas d'id dans $raw"; exit 1 }
  $ids[$t.id] = $id
  Write-Host "$($t.id) -> $id"
}
Write-Host "Termine : $($ids.Count) cartes."
