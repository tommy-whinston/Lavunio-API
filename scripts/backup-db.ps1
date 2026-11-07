param(
    [string]$DatabaseUrl = $env:DATABASE_URL,
    [string]$OutputDir = "",
    [int]$KeepDays = 7
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $DatabaseUrl) {
    $envFile = Join-Path $repoRoot ".env"
    if (Test-Path -LiteralPath $envFile) {
        $line = Get-Content -LiteralPath $envFile | Where-Object { $_ -match '^DATABASE_URL=' } | Select-Object -First 1
        if ($line) {
            $DatabaseUrl = $line.Substring("DATABASE_URL=".Length).Trim()
        }
    }
}
if (-not $DatabaseUrl) {
    throw "DATABASE_URL не задан. Передай -DatabaseUrl или заполни .env."
}
if (-not $OutputDir) {
    $OutputDir = Join-Path $repoRoot "backups"
}

function Find-PgTool([string]$Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }
    $found = Get-ChildItem "C:\Program Files\PostgreSQL\*\bin\$Name.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if ($found) {
        return $found.FullName
    }
    throw "$Name не найден. Добавь bin PostgreSQL в PATH."
}

$pgDump = Find-PgTool "pg_dump"
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$file = Join-Path $OutputDir "lavunio-$stamp.dump"

Write-Host "Бэкап: $file"
& $pgDump --dbname=$DatabaseUrl --format=custom --no-owner --no-privileges --file=$file
if ($LASTEXITCODE -ne 0) {
    if (Test-Path -LiteralPath $file) {
        Remove-Item -LiteralPath $file -Force
    }
    throw "pg_dump завершился с кодом $LASTEXITCODE"
}

if ($KeepDays -gt 0) {
    $cutoff = (Get-Date).AddDays(-$KeepDays)
    Get-ChildItem -LiteralPath $OutputDir -Filter "lavunio-*.dump" |
        Where-Object { $_.LastWriteTime -lt $cutoff } |
        ForEach-Object {
            Remove-Item -LiteralPath $_.FullName -Force
            Write-Host "Удалён старый бэкап: $($_.Name)"
        }
}

$size = (Get-Item -LiteralPath $file).Length
Write-Host ("Готово: {0} ({1:N1} МБ)" -f $file, ($size / 1MB))
