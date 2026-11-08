# check_metrics.ps1 — compare current metrics against baselines
# For each metrics/<suite>.json, ensure pass count hasn't regressed.
# If baseline doesn't exist, create it from current results.

$ErrorActionPreference = "Stop"
$metricsDir = Join-Path $PSScriptRoot "..\metrics"
$metricsDir = Resolve-Path $metricsDir

if (-not (Test-Path $metricsDir)) {
    Write-Host "No metrics directory found."
    exit 0
}

$suites = Get-ChildItem -Path $metricsDir -Filter "*.json" | Where-Object { $_.Name -notlike "*.baseline.json" }

if ($suites.Count -eq 0) {
    Write-Host "No metric files found."
    exit 0
}

$regressed = $false

foreach ($suite in $suites) {
    $name = $suite.BaseName
    $currentPath = $suite.FullName
    $baselinePath = Join-Path $metricsDir "$name.baseline.json"

    $current = Get-Content $currentPath -Raw | ConvertFrom-Json

    if (-not (Test-Path $baselinePath)) {
        # No baseline yet — create it from current results
        Copy-Item $currentPath $baselinePath
        Write-Host "Created baseline for '${name}'."
    } else {
        $baseline = Get-Content $baselinePath -Raw | ConvertFrom-Json

        if ($current.pass -lt $baseline.pass) {
            Write-Host "REGRESSION ${name}: $($baseline.pass) -> $($current.pass)"
            $regressed = $true
        } elseif ($current.pass -gt $baseline.pass) {
            Write-Host "Improved '${name}': $($baseline.pass) -> $($current.pass) (baseline not auto-updated)."
        } else {
            Write-Host "'${name}' unchanged at $($current.pass) passes."
        }
    }
}

if ($regressed) {
    exit 1
}

Write-Host "All metrics OK."
exit 0
