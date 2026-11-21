# Publishes the Windows app for release: unpackaged, self-contained (.NET and the Windows App SDK
# included, nothing to install), zipped as
#   <OutputDir>\OmarchyThemes-<version>-windows-<arch>.zip
# with an "Omarchy Themes" folder inside holding OmarchyThemes.exe.
#
# Usage (PowerShell 7 on Windows, from anywhere):
#   ./windows/tools/Publish-App.ps1 -Runtime win-x64 -OutputDir dist
#   ./windows/tools/Publish-App.ps1 -Runtime win-arm64 -OutputDir dist
#Requires -Version 7
param(
    [ValidateSet('win-x64', 'win-arm64')]
    [string]$Runtime = 'win-x64',
    [string]$OutputDir = 'dist'
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$windows = Resolve-Path (Join-Path $PSScriptRoot '..')
$project = Join-Path $windows 'src\OmarchyThemes.App\OmarchyThemes.App.csproj'
$version = (Select-Xml -Path (Join-Path $windows 'Directory.Build.props') -XPath '//Version').Node.InnerText
if (-not $version) { throw 'No <Version> in windows/Directory.Build.props' }

$platform = if ($Runtime -eq 'win-arm64') { 'ARM64' } else { 'x64' }
$arch = $Runtime -replace '^win-', ''

New-Item -ItemType Directory -Force $OutputDir | Out-Null
$OutputDir = (Resolve-Path $OutputDir).Path
$zip = Join-Path $OutputDir "OmarchyThemes-$version-windows-$arch.zip"

$staging = Join-Path ([System.IO.Path]::GetTempPath()) "omatheme-publish-$([guid]::NewGuid())"
$appDir = Join-Path $staging 'Omarchy Themes'
try {
    dotnet publish $project `
        --configuration Release `
        --runtime $Runtime `
        --self-contained true `
        -p:Platform=$platform `
        --output $appDir

    # What the app can't start without: the exe, the WinUI resources index and the Windows App SDK runtime.
    foreach ($pattern in 'OmarchyThemes.exe', '*.pri', 'Microsoft.WindowsAppRuntime*.dll', 'Microsoft.UI.Xaml*.dll') {
        if (-not (Get-ChildItem -Path $appDir -Filter $pattern -File)) {
            throw "The publish output has no $pattern"
        }
    }
    $exe = Get-Item (Join-Path $appDir 'OmarchyThemes.exe')
    if (-not $exe.VersionInfo.ProductVersion.StartsWith($version)) {
        throw "OmarchyThemes.exe says $($exe.VersionInfo.ProductVersion), the repo says $version"
    }
    # Debug symbols stay out of the download.
    Get-ChildItem -Path $appDir -Filter '*.pdb' -Recurse | Remove-Item

    if (Test-Path $zip) { Remove-Item $zip }
    Compress-Archive -Path $appDir -DestinationPath $zip -CompressionLevel Optimal
    Write-Host "Built Omarchy Themes $version for $Runtime"
    $zip
}
finally {
    if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
}
