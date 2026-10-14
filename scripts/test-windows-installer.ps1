param([Parameter(Mandatory)][string]$ArtifactDirectory)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Installer([string]$File, [string]$Arguments) {
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $File
    $info.Arguments = $Arguments
    $info.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($info)
    try {
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            throw "Installer timed out: $File"
        }
        Write-Host "Exit code: $($process.ExitCode)"
        if ($process.ExitCode -ne 0) {
            throw "Installer failed with exit code $($process.ExitCode): $File"
        }
    } finally {
        $process.Dispose()
    }
}

$installers = @(Get-ChildItem $ArtifactDirectory -Recurse -Filter '*-setup.exe')
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer' }
$installer = $installers[0]
Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber | Format-List
Get-FileHash $installer.FullName -Algorithm SHA256 | Format-List
Get-AuthenticodeSignature $installer.FullName | Select-Object Status, StatusMessage | Format-List
& 7z t $installer.FullName
if ($LASTEXITCODE -ne 0) { throw 'Installer archive integrity check failed' }

$originalTemp = $env:TEMP
$originalTmp = $env:TMP
$root = Join-Path $env:RUNNER_TEMP ('clicove-installer-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $root | Out-Null
try {
    foreach ($case in @('default', 'isolated')) {
        if ($case -eq 'isolated') {
            $env:TEMP = Join-Path $root 'temporary-files'
            $env:TMP = $env:TEMP
            New-Item -ItemType Directory -Path $env:TEMP | Out-Null
        }
        $installDirectory = Join-Path $root "installed-$case"
        Write-Host "Testing $case temporary directory: TEMP=$env:TEMP TMP=$env:TMP"
        Invoke-Installer $installer.FullName "/S /D=$installDirectory"
        $application = Join-Path $installDirectory 'clicove.exe'
        if (-not (Test-Path $application)) { throw "Application missing: $application" }
        Get-Item $application | Select-Object FullName, Length | Format-List
        Invoke-Installer (Join-Path $installDirectory 'uninstall.exe') "/S _?=$installDirectory"
        if (Test-Path $application) { throw 'Uninstall left the application executable behind' }
        "- PASS: NSIS install and uninstall with $case temporary directory" | Add-Content $env:GITHUB_STEP_SUMMARY
    }
} finally {
    $env:TEMP = $originalTemp
    $env:TMP = $originalTmp
}
