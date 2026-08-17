Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. "$PSScriptRoot\..\site\install.ps1"

function Assert-RebinderTest {
  param([Parameter(Mandatory)][bool] $Condition, [Parameter(Mandatory)][string] $Message)
  if (-not $Condition) { throw "rebinder installer test: $Message" }
}

$root = Join-Path ([System.IO.Path]::GetTempPath()) "rebinder-installer-test-$([guid]::NewGuid().ToString('N'))"
try {
  $version = "0.20260817.0"
  $tag = "v$version"
  $target = "x86_64-pc-windows-msvc"
  $stagingName = "rebinder-$tag-$target"
  $releaseDirectory = Join-Path $root "release"
  $staging = Join-Path $root $stagingName
  $installDirectory = Join-Path $root "bin"
  New-Item -ItemType Directory -Force -Path $releaseDirectory, $staging | Out-Null

  $source = @"
using System;
public static class RebinderInstallerFixture
{
    public static int Main(string[] args)
    {
        if (args.Length == 1 && args[0] == "--version")
        {
            Console.WriteLine("rebinder $version");
            return 0;
        }
        return 2;
    }
}
"@
  $sourcePath = Join-Path $root "fixture.cs"
  Set-Content -LiteralPath $sourcePath -Value $source
  $compiler = @(
    (Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\csc.exe"),
    (Join-Path $env:WINDIR "Microsoft.NET\Framework\v4.0.30319\csc.exe")
  ) | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
  if (-not $compiler) { throw "Windows installer test requires csc.exe" }
  & $compiler /nologo /target:exe "/out:$(Join-Path $staging 'rebinder.exe')" $sourcePath
  if ($LASTEXITCODE -ne 0) { throw "fixture compilation failed" }

  [ordered]@{
    name = "rebinder"
    version = $version
    buildId = "$version+sha.000000000000"
    tag = $tag
    commit = "0000000000000000000000000000000000000000"
    commitDate = "2026-08-17"
    target = $target
  } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $staging "release.json")

  $archiveName = "$stagingName.zip"
  $archivePath = Join-Path $releaseDirectory $archiveName
  Compress-Archive -Path $staging -DestinationPath $archivePath
  $hash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
  Set-Content -LiteralPath (Join-Path $releaseDirectory "SHA256SUMS") -Value "$hash  $archiveName"

  $script:RebinderMockReleaseDirectory = $releaseDirectory
  function Invoke-RebinderDownload {
    param([Parameter(Mandatory)][string] $Uri, [Parameter(Mandatory)][string] $OutFile)
    $name = [System.IO.Path]::GetFileName(([uri] $Uri).AbsolutePath)
    Copy-Item -LiteralPath (Join-Path $script:RebinderMockReleaseDirectory $name) -Destination $OutFile
  }

  Invoke-RebinderInstaller -RequestedVersion $tag -InstallDirectory $installDirectory | Out-Null
  $destination = Join-Path $installDirectory "rebinder.exe"
  Assert-RebinderTest -Condition (Test-Path -LiteralPath $destination -PathType Leaf) -Message "binary missing"
  $reported = (& $destination --version | Out-String).Trim()
  Assert-RebinderTest -Condition ($reported -eq "rebinder $version") -Message "version smoke test failed"

  $installedHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
  Add-Content -LiteralPath $archivePath -Value "corruption"
  $rejected = $false
  try {
    Invoke-RebinderInstaller -RequestedVersion $tag -InstallDirectory $installDirectory | Out-Null
  } catch {
    $rejected = $_.Exception.Message -like "*checksum verification failed*"
  }
  Assert-RebinderTest -Condition $rejected -Message "corrupted archive was accepted"
  Assert-RebinderTest `
    -Condition ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -eq $installedHash) `
    -Message "failed refresh changed installed binary"

  Write-Output "Windows installer acceptance passed."
} finally {
  if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
