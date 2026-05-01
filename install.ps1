param(
    [string]$Version,
    [string]$Repo = "DanielHauge/h5v",
    [string]$InstallDir = $env:H5V_INSTALL_DIR,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

if (-not $InstallDir) {
    $InstallDir = Join-Path $HOME ".local\bin"
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

    Write-Host "Installed h5v to $InstallDir\h5v.exe"

    $pathEntries = ($env:PATH -split ';') | Where-Object { $_ }
    if ($pathEntries -notcontains $InstallDir) {
        Write-Warning "$InstallDir is not currently on PATH."
    }
} finally {
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
