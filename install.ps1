param(
    [string]$Version,
    [string]$Repo = "DanielHauge/h5v",
    [string]$InstallDir = $env:H5V_INSTALL_DIR,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

function Get-DefaultInstallDir() {
    if ($env:LOCALAPPDATA) {
        return (Join-Path $env:LOCALAPPDATA "Programs\h5v\bin")
    }

    if ($env:USERPROFILE) {
        return (Join-Path $env:USERPROFILE "AppData\Local\Programs\h5v\bin")
    }

    return (Join-Path $HOME "AppData\Local\Programs\h5v\bin")
}

function Test-PathEntry([string]$Candidate, [string[]]$Entries) {
    $normalizedCandidate = $Candidate.TrimEnd('\')
    foreach ($entry in $Entries) {
        if ($entry.TrimEnd('\') -ieq $normalizedCandidate) {
            return $true
        }
    }

    return $false
}

function Add-UserPathEntry([string]$PathEntry) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $userEntries = if ($userPath) {
        ($userPath -split ';') | Where-Object { $_ }
    } else {
        @()
    }

    if (-not (Test-PathEntry $PathEntry $userEntries)) {
        $newUserPath = if ($userEntries.Count -gt 0) {
            ($userEntries + $PathEntry) -join ';'
        } else {
            $PathEntry
        }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
        Write-Host "Added $PathEntry to your user PATH."
    }

    $sessionEntries = ($env:PATH -split ';') | Where-Object { $_ }
    if (-not (Test-PathEntry $PathEntry $sessionEntries)) {
        $env:PATH = if ($env:PATH) { "$env:PATH;$PathEntry" } else { $PathEntry }
    }
}

if (-not $InstallDir) {
    $InstallDir = Get-DefaultInstallDir
}

function Normalize-Version([string]$Value) {
    if ($Value.StartsWith("v")) {
        return $Value.Substring(1)
    }
    return $Value
}

function Get-LatestTag([string]$Repository) {
    $headers = @{ "User-Agent" = "h5v-installer" }
    return (Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$Repository/releases/latest").tag_name
}

$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
switch ($arch) {
    "X64" { $target = "x86_64-pc-windows-msvc" }
    "Arm64" { throw "Windows ARM64 installers are not published yet." }
    default { throw "Unsupported Windows architecture: $arch" }
}

if ($Version) {
    $normalizedVersion = Normalize-Version $Version
    $tag = "v$normalizedVersion"
} else {
    $tag = Get-LatestTag $Repo
    $normalizedVersion = Normalize-Version $tag
}

$archive = "h5v-$target-v$normalizedVersion.zip"
$checksum = "$archive.sha256"
$archiveUrl = "https://github.com/$Repo/releases/download/$tag/$archive"
$checksumUrl = "https://github.com/$Repo/releases/download/$tag/$checksum"

if ($DryRun) {
    Write-Host "Repository: $Repo"
    Write-Host "Version: $normalizedVersion"
    Write-Host "Target: $target"
    Write-Host "Install dir: $InstallDir"
    Write-Host "Archive URL: $archiveUrl"
    return
}

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("h5v-install-" + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

try {
    $archivePath = Join-Path $tmpDir $archive
    $checksumPath = Join-Path $tmpDir $checksum

    Invoke-WebRequest -Headers @{ "User-Agent" = "h5v-installer" } -Uri $archiveUrl -OutFile $archivePath
    Invoke-WebRequest -Headers @{ "User-Agent" = "h5v-installer" } -Uri $checksumUrl -OutFile $checksumPath

    $expectedHash = (Get-Content $checksumPath -Raw).Split()[0].ToLowerInvariant()
    $actualHash = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expectedHash -ne $actualHash) {
        throw "SHA256 mismatch for $archive"
    }

    Expand-Archive -Path $archivePath -DestinationPath $tmpDir -Force
    $sourceDir = Join-Path $tmpDir "h5v-$target-v$normalizedVersion"

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $sourceDir "h5v.exe") (Join-Path $InstallDir "h5v.exe") -Force
    Add-UserPathEntry $InstallDir

    Write-Host "Installed h5v to $InstallDir\h5v.exe"
} finally {
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
