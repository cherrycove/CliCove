param([Parameter(Mandatory)][string]$ArtifactDirectory)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Run this test only on a disposable GitHub Actions runner' }

$userName = 'CliCoveTest'
$password = ConvertTo-SecureString ('aA1!' + [guid]::NewGuid().ToString('N')) -AsPlainText -Force
$user = New-LocalUser -Name $userName -Password $password
try {
    $usersGroup = Get-LocalGroup -SID 'S-1-5-32-545'
    Add-LocalGroupMember -Group $usersGroup -Member $user
    $root = Join-Path $env:PUBLIC ('CliCoveInstallerTest-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $root | Out-Null
    & icacls $root /grant ('*{0}:(OI)(CI)M' -f $user.SID.Value)
    if ($LASTEXITCODE -ne 0) { throw 'Could not grant the test account access to test files' }
    Copy-Item $ArtifactDirectory (Join-Path $root 'installers') -Recurse
    Copy-Item (Join-Path $PSScriptRoot 'test-windows-installer.ps1') $root
    @'
$ErrorActionPreference = 'Stop'
try {
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [System.Security.Principal.WindowsPrincipal]::new($identity)
    if ($principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'The test must run without administrator privileges'
    }
    $profileKey = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\' + $identity.User.Value
    $env:USERPROFILE = [Environment]::ExpandEnvironmentVariables((Get-ItemProperty $profileKey).ProfileImagePath)
    $env:USERNAME = $identity.Name.Split('\')[-1]
    $env:HOMEDRIVE = Split-Path $env:USERPROFILE -Qualifier
    $env:HOMEPATH = $env:USERPROFILE.Substring($env:HOMEDRIVE.Length)
    $env:LOCALAPPDATA = Join-Path $env:USERPROFILE 'AppData\Local'
    $env:APPDATA = Join-Path $env:USERPROFILE 'AppData\Roaming'
    $env:TEMP = Join-Path $env:LOCALAPPDATA 'Temp'
    $env:TMP = $env:TEMP
    $env:RUNNER_TEMP = $env:TEMP
    New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
    $env:GITHUB_STEP_SUMMARY = Join-Path $PSScriptRoot 'standard-user-summary.txt'
    & (Join-Path $PSScriptRoot 'test-windows-installer.ps1') -ArtifactDirectory (Join-Path $PSScriptRoot 'installers')
    exit 0
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
'@ | Set-Content (Join-Path $root 'run.ps1')
    $credential = [pscredential]::new("$env:COMPUTERNAME\$userName", $password)
    $process = Start-Process -FilePath (Get-Command pwsh).Source `
        -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$root\run.ps1`"" `
        -Credential $credential -LoadUserProfile -WorkingDirectory $root `
        -RedirectStandardOutput "$root\stdout.txt" -RedirectStandardError "$root\stderr.txt" -PassThru
    try {
        if (-not $process.WaitForExit(300000)) {
            $process.Kill($true)
            throw 'Standard user installation test timed out'
        }
        Get-Content "$root\stdout.txt", "$root\stderr.txt" | Write-Host
        if ($process.ExitCode -ne 0) { throw "Standard user test failed: $($process.ExitCode)" }
        Get-Content "$root\standard-user-summary.txt" | Add-Content $env:GITHUB_STEP_SUMMARY
    } finally {
        $process.Dispose()
    }
} finally {
    Remove-LocalUser -Name $userName
}
