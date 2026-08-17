[CmdletBinding()]
param(
  [string] $Version,
  [string] $To
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$script:RebinderRepository = "bahadirarda/rebinder"
[System.Net.ServicePointManager]::SecurityProtocol = `
  [System.Net.ServicePointManager]::SecurityProtocol -bor [System.Net.SecurityProtocolType]::Tls12

function Write-RebinderMessage {
  param([Parameter(Mandatory)][string] $Message)
  Write-Output "rebinder: $Message"
}

function Throw-RebinderError {
  param([Parameter(Mandatory)][string] $Message)
  throw "rebinder: error: $Message"
}

function Invoke-RebinderDownload {
  param(
    [Parameter(Mandatory)][string] $Uri,
    [Parameter(Mandatory)][string] $OutFile
  )
  for ($attempt = 1; $attempt -le 3; $attempt++) {
    try {
      Invoke-WebRequest -Uri $Uri -OutFile $OutFile -UseBasicParsing
      return
    } catch {
      if ($attempt -eq 3) { throw }
      Start-Sleep -Seconds $attempt
    }
  }
}

function Resolve-RebinderLatestTag {
  $headers = @{
    Accept = "application/vnd.github+json"
    "User-Agent" = "rebinder-installer"
    "X-GitHub-Api-Version" = "2022-11-28"
  }
  $release = Invoke-RestMethod `
    -Uri "https://api.github.com/repos/$script:RebinderRepository/releases/latest" `
    -Headers $headers
  return [string] $release.tag_name
}

function Test-RebinderReparsePoint {
  param([Parameter(Mandatory)][System.IO.FileSystemInfo] $Item)
  return ($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
}

function Remove-RebinderTemporaryPath {
  param([AllowNull()][string] $Path)
  if ($Path -and (Test-Path -LiteralPath $Path)) {
    Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue
  }
}

function Invoke-RebinderInstaller {
  [CmdletBinding()]
  param(
    [string] $RequestedVersion,
    [string] $InstallDirectory
  )

  if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    Throw-RebinderError "this installer supports Windows; use install.sh on Linux or macOS"
  }
  $architecture = if ($env:PROCESSOR_ARCHITEW6432) {
    $env:PROCESSOR_ARCHITEW6432
  } else {
    $env:PROCESSOR_ARCHITECTURE
  }
  if ($architecture -notin @("AMD64", "x86_64")) {
    Throw-RebinderError "unsupported Windows architecture: $architecture"
  }

  if (-not $RequestedVersion) {
    $RequestedVersion = if ($env:REBINDER_VERSION) { $env:REBINDER_VERSION } else { "latest" }
  }
  $releaseTag = if ($RequestedVersion -eq "latest") {
    Resolve-RebinderLatestTag
  } elseif ($RequestedVersion.StartsWith("v")) {
    $RequestedVersion
  } else {
    "v$RequestedVersion"
  }
  if ($releaseTag -notmatch '^v0\.[0-9]{8}\.(0|[1-9][0-9]*)$') {
    Throw-RebinderError "release version must match v0.YYYYMMDD.REVISION: $releaseTag"
  }

  if (-not $InstallDirectory) {
    $InstallDirectory = if ($env:REBINDER_INSTALL_DIR) {
      $env:REBINDER_INSTALL_DIR
    } elseif ($env:LOCALAPPDATA) {
      Join-Path $env:LOCALAPPDATA "rebinder\bin"
    } else {
      Throw-RebinderError "LOCALAPPDATA is not set; pass -To or REBINDER_INSTALL_DIR"
    }
  }
  $InstallDirectory = [System.IO.Path]::GetFullPath($InstallDirectory)

  $target = "x86_64-pc-windows-msvc"
  $archive = "rebinder-$releaseTag-$target.zip"
  $downloadRoot = "https://github.com/$script:RebinderRepository/releases/download/$releaseTag"
  $temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rebinder-install-$([guid]::NewGuid().ToString('N'))"
  $binaryTemporary = $null
  $binaryBackup = $null
  $destination = $null
  $activated = $false

  try {
    New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
    $archivePath = Join-Path $temporaryRoot $archive
    $checksumPath = Join-Path $temporaryRoot "SHA256SUMS"
    Write-RebinderMessage "downloading $releaseTag for $target"
    Invoke-RebinderDownload -Uri "$downloadRoot/$archive" -OutFile $archivePath
    Invoke-RebinderDownload -Uri "$downloadRoot/SHA256SUMS" -OutFile $checksumPath

    $matchingHashes = @()
    foreach ($line in Get-Content -LiteralPath $checksumPath) {
      if ($line -match '^(?<Hash>[0-9a-fA-F]{64})\s+\*?(?<Name>.+)$' -and $Matches.Name -eq $archive) {
        $matchingHashes += $Matches.Hash.ToLowerInvariant()
      }
    }
    if ($matchingHashes.Count -ne 1) {
      Throw-RebinderError "release checksum is missing or ambiguous for $archive"
    }
    Write-RebinderMessage "verifying SHA-256 checksum"
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $matchingHashes[0]) {
      Throw-RebinderError "checksum verification failed"
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $temporaryRoot
    $staging = Join-Path $temporaryRoot "rebinder-$releaseTag-$target"
    $sourceBinary = Join-Path $staging "rebinder.exe"
    $sourceMetadata = Join-Path $staging "release.json"
    if (-not (Test-Path -LiteralPath $sourceBinary -PathType Leaf)) {
      Throw-RebinderError "release archive does not contain rebinder.exe"
    }
    if (-not (Test-Path -LiteralPath $sourceMetadata -PathType Leaf)) {
      Throw-RebinderError "release archive does not contain release.json"
    }
    $metadata = Get-Content -LiteralPath $sourceMetadata -Raw | ConvertFrom-Json
    if (
      $metadata.name -ne "rebinder" -or
      $metadata.version -ne $releaseTag.Substring(1) -or
      $metadata.tag -ne $releaseTag -or
      $metadata.target -ne $target
    ) {
      Throw-RebinderError "release metadata does not match the requested artifact"
    }

    New-Item -ItemType Directory -Force -Path $InstallDirectory | Out-Null
    $destination = Join-Path $InstallDirectory "rebinder.exe"
    $existing = Get-Item -LiteralPath $destination -Force -ErrorAction SilentlyContinue
    if ($existing -and ((Test-RebinderReparsePoint -Item $existing) -or $existing.PSIsContainer)) {
      Throw-RebinderError "unsafe executable destination: $destination"
    }
    $nonce = [guid]::NewGuid().ToString('N')
    $binaryTemporary = Join-Path $InstallDirectory ".rebinder.$nonce.tmp.exe"
    $binaryBackup = Join-Path $InstallDirectory ".rebinder.$nonce.backup.exe"
    Copy-Item -LiteralPath $sourceBinary -Destination $binaryTemporary

    if (Test-Path -LiteralPath $destination) {
      Move-Item -LiteralPath $destination -Destination $binaryBackup
    }
    Move-Item -LiteralPath $binaryTemporary -Destination $destination
    $binaryTemporary = $null
    $activated = $true

    $reportedVersion = (& $destination --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $reportedVersion -ne "rebinder $($releaseTag.Substring(1))") {
      Throw-RebinderError "installed executable reported an unexpected version: $reportedVersion"
    }

    $activated = $false
    Remove-RebinderTemporaryPath -Path $binaryBackup
    $binaryBackup = $null
    Write-RebinderMessage "installed $releaseTag to $destination"
    $pathEntries = @($env:PATH -split ';' | ForEach-Object { $_.TrimEnd('\') })
    if ($pathEntries -notcontains $InstallDirectory.TrimEnd('\')) {
      Write-RebinderMessage "add $InstallDirectory to PATH before running rebinder"
    }
  } catch {
    if ($activated -and $destination -and (Test-Path -LiteralPath $destination)) {
      Remove-Item -LiteralPath $destination -Force
    }
    if ($binaryBackup -and (Test-Path -LiteralPath $binaryBackup)) {
      Move-Item -LiteralPath $binaryBackup -Destination $destination
      $binaryBackup = $null
    }
    throw
  } finally {
    Remove-RebinderTemporaryPath -Path $binaryTemporary
    Remove-RebinderTemporaryPath -Path $temporaryRoot
  }
}

if ($MyInvocation.InvocationName -ne '.') {
  Invoke-RebinderInstaller -RequestedVersion $Version -InstallDirectory $To
}
