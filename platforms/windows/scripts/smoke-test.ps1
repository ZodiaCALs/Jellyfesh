<#
.SYNOPSIS
Starts the published Windows app with --smoke-test and reports how it went.

.DESCRIPTION
Runs Jellyfesh.exe --smoke-test (docs/windows.md, "When Jellyfesh does not start"): the app starts
its core and its main window, opens every page, closes, and exits with 0; anything else is a failure
(1 start-up failed, 2 timed out, 3 another Jellyfesh took the start, 4 shutting down hung). This is
what CI runs after scripts/build.ps1 (.github/workflows/ci.yml, "Windows app starts").

Two runs:
  1. from the publish folder, with its own data, cache and log folders (JF_DATA_DIR, JF_CACHE_DIR,
     JF_LOG_DIR), or with the user's real folders when -DefaultFolders is given (CI, whose user
     profile is new);
  2. from a copy of the app in a folder whose path has spaces and non-ASCII characters, with the data,
     cache and log folders on such paths too, as for a person whose Windows user name is not ASCII.

After each run the app's log is printed; after a failed one also the Windows Application event log's
.NET Runtime, Application Error and Windows Error Reporting entries and any about Jellyfesh (crashes
.NET never sees). The logs are copied to <WorkDir>\logs. Exits 1
when any run failed.

.PARAMETER App
The published app folder, the one with Jellyfesh.exe. Default: platforms/windows/publish/win-x64/Jellyfesh.

.PARAMETER WorkDir
Where the copies, folders and logs go. Default: jellyfesh-smoke in RUNNER_TEMP or the temporary folder.

.PARAMETER TimeoutSeconds
How long one run may take before it is ended and counted as failed. Default: 420 (the app's own
limits are 3 minutes to open every page and 2 to shut down, which waits for plugin installs).

.PARAMETER DefaultFolders
Run 1 uses the user's own data, cache and log folders, as an installed app does.

.PARAMETER InPlaceOnly
Only run 1, from -App as it is (for an app Setup has installed).
#>
[CmdletBinding()]
param(
    [string] $App,

    [string] $WorkDir,

    [int] $TimeoutSeconds = 420,

    [switch] $DefaultFolders,

    [switch] $InPlaceOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Windows = Split-Path -Parent $PSScriptRoot
if (-not $App) { $App = Join-Path $Windows 'publish\win-x64\Jellyfesh' }
$App = (Resolve-Path $App).Path
if (-not (Test-Path (Join-Path $App 'Jellyfesh.exe'))) { throw "no Jellyfesh.exe in $App" }
if (-not $WorkDir) {
    $temp = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [IO.Path]::GetTempPath() }
    $WorkDir = Join-Path $temp 'jellyfesh-smoke'
}
if (Test-Path $WorkDir) { Remove-Item $WorkDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path (Join-Path $WorkDir 'logs') | Out-Null

# Non-ASCII from code points, so this file's own encoding cannot change them: Chinese (ce shi,
# shu ju), a German U-umlaut and a French e-acute, with spaces.
$unicodeApp = 'Jellyfesh ' + [char]0x6D4B + [char]0x8BD5 + ' ' + [char]0x00DC + 'ber ' + [char]0x00FC
$unicodeData = [string][char]0x6570 + [char]0x636E + ' Donn' + [char]0x00E9 + 'es'

$ExitMeanings = @{
    0 = 'every page opened'
    1 = 'start-up failed'
    2 = 'timed out'
    3 = 'another Jellyfesh took the start'
    4 = 'shutting down hung'
}

function Invoke-SmokeRun {
    param(
        [Parameter(Mandatory)] [string] $Name,
        [Parameter(Mandatory)] [string] $Folder,
        [hashtable] $Environment = @{},
        [Parameter(Mandatory)] [string] $Log
    )

    Write-Host "== $Name" -ForegroundColor Cyan
    Write-Host "   $Folder\Jellyfesh.exe --smoke-test"
    $start = Get-Date
    $info = [Diagnostics.ProcessStartInfo]::new((Join-Path $Folder 'Jellyfesh.exe'), '--smoke-test')
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $Folder
    foreach ($key in $Environment.Keys) {
        $info.Environment[$key] = $Environment[$key]
        Write-Host "   $key=$($Environment[$key])"
    }

    $process = [Diagnostics.Process]::Start($info)
    $ok = $false
    if ($process.WaitForExit($TimeoutSeconds * 1000)) {
        $process.WaitForExit() # lets the exit code settle
        $code = $process.ExitCode
        $meaning = if ($ExitMeanings.ContainsKey($code)) { $ExitMeanings[$code] } else { 'crashed' }
        $seconds = [int]((Get-Date) - $start).TotalSeconds
        Write-Host "   exit code $code ($meaning) after $seconds s"
        $ok = $code -eq 0
    }
    else {
        Write-Host "   still running after $TimeoutSeconds s: ending it"
        $process.Kill($true)
    }

    Write-Host "-- the log ($Log)"
    if (Test-Path $Log) {
        Get-Content -LiteralPath $Log -Encoding utf8 | ForEach-Object { Write-Host "   $_" }
        Copy-Item -LiteralPath $Log (Join-Path $WorkDir "logs\$($Name -replace '[^A-Za-z0-9]+', '-').log")
    }
    else {
        Write-Host '   (no log was written)'
    }

    if (-not $ok) {
        # Crashes outside .NET's reach (a fail-fast in WinUI, a native exception) leave only these.
        Write-Host '-- the Application event log since the run started (.NET Runtime, Application Error, Windows Error Reporting, or about Jellyfesh)'
        $crashProviders = @('.NET Runtime', 'Application Error', 'Windows Error Reporting')
        try {
            $events = @(Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = $start } -ErrorAction Stop |
                Where-Object { $_.ProviderName -in $crashProviders -or $_.Message -match 'Jellyfesh' })
            if ($events.Count -eq 0) { Write-Host '   (none)' }
            $events | ForEach-Object { Write-Host "   [$($_.TimeCreated)] $($_.ProviderName) $($_.Id): $($_.Message)" }
        }
        catch {
            Write-Host "   (none: $($_.Exception.Message))"
        }
        Write-Host "::error title=Windows smoke test::$Name failed; see the log above"
    }

    return $ok
}

$results = @()

# 1. From the publish folder.
if ($DefaultFolders) {
    $log = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Jellyfesh\logs\app.log'
    if (Test-Path $log) { Remove-Item $log -Force }
    $results += Invoke-SmokeRun -Name 'publish folder, default folders' -Folder $App -Log $log
}
else {
    $folders = Join-Path $WorkDir 'plain'
    $results += Invoke-SmokeRun -Name 'publish folder' -Folder $App -Log (Join-Path $folders 'logs\app.log') -Environment @{
        JF_DATA_DIR  = Join-Path $folders 'data'
        JF_CACHE_DIR = Join-Path $folders 'cache'
        JF_LOG_DIR   = Join-Path $folders 'logs'
    }
}

# 2. From a path with spaces and non-ASCII characters, with its folders on such paths too.
if (-not $InPlaceOnly) {
    $copy = Join-Path $WorkDir $unicodeApp
    Copy-Item -LiteralPath $App -Destination $copy -Recurse
    $folders = Join-Path $WorkDir $unicodeData
    $results += Invoke-SmokeRun -Name 'non-ASCII path' -Folder $copy -Log (Join-Path $folders 'logs\app.log') -Environment @{
        JF_DATA_DIR  = Join-Path $folders 'data'
        JF_CACHE_DIR = Join-Path $folders 'cache'
        JF_LOG_DIR   = Join-Path $folders 'logs'
    }
}

$failed = @($results | Where-Object { -not $_ }).Count
if ($failed -gt 0) {
    Write-Host "$failed of $($results.Count) smoke runs failed" -ForegroundColor Red
    exit 1
}
Write-Host "All $($results.Count) smoke runs passed" -ForegroundColor Green
exit 0
