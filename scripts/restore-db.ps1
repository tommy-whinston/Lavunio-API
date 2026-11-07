param(
    [Parameter(Mandatory = $true)]
    [string]$Dump,
    [string]$DatabaseUrl = $env:DATABASE_URL
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
if (-not (Test-Path -LiteralPath $Dump)) {
    throw "Файл бэкапа не найден: $Dump"
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

$pgRestore = Find-PgTool "pg_restore"
Write-Host "Восстановление: $Dump"
Write-Host "База: $DatabaseUrl"
Write-Host "Внимание: существующие объекты в базе будут удалены и заменены."
& $pgRestore --dbname=$DatabaseUrl --clean --if-exists --no-owner --no-privileges $Dump
if ($LASTEXITCODE -ne 0) {
    throw "pg_restore завершился с кодом $LASTEXITCODE"
}
Write-Host "Готово."
