# Renders the app icon (same geometry as design/AppIcon.svg) into the WinUI app's
# Assets folder: AppIcon.png (256px) and a multi-resolution AppIcon.ico.
# Usage (PowerShell 7 on Windows):  ./tools/Generate-AppIcon.ps1
#Requires -Version 7
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$assets = Join-Path $PSScriptRoot '..\src\OmarchyThemes.App\Assets'
New-Item -ItemType Directory -Force $assets | Out-Null

function New-RoundedRect([float]$x, [float]$y, [float]$w, [float]$h, [float]$r) {
    $p = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    $p
}

function Get-IconPng([int]$size) {
    $bmp = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)
    $s = $size / 1024.0
    $g.ScaleTransform($s, $s)

    $bg = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
        [System.Drawing.PointF]::new(0, 64), [System.Drawing.PointF]::new(0, 960),
        [System.Drawing.ColorTranslator]::FromHtml('#24283b'),
        [System.Drawing.ColorTranslator]::FromHtml('#13141c'))
    $g.FillPath($bg, (New-RoundedRect 64 64 896 896 208))

    $tiles = @(
        @(192, 192, 296, 640, '#7aa2f7'),
        @(536, 192, 296, 296, '#bb9af7'),
        @(536, 536, 296, 296, '#9ece6a')
    )
    foreach ($t in $tiles) {
        $brush = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml($t[4]))
        $g.FillPath($brush, (New-RoundedRect $t[0] $t[1] $t[2] $t[3] 56))
    }
    $g.Dispose()

    $ms = [System.IO.MemoryStream]::new()
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    , $ms.ToArray()
}

# 256px PNG used by XAML (title bar, welcome, about).
[System.IO.File]::WriteAllBytes((Join-Path $assets 'AppIcon.png'), (Get-IconPng 256))

# Multi-resolution ICO with PNG-compressed entries (supported since Windows Vista).
$sizes = 16, 20, 24, 32, 40, 48, 64, 256
$images = foreach ($sz in $sizes) { , (Get-IconPng $sz) }
$out = [System.IO.MemoryStream]::new()
$w = [System.IO.BinaryWriter]::new($out)
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
    $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$images[$i].Length); $w.Write([uint32]$offset)
    $offset += $images[$i].Length
}
foreach ($img in $images) { $w.Write($img) }
$w.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $assets 'AppIcon.ico'), $out.ToArray())

Write-Host "Wrote AppIcon.png and AppIcon.ico to $assets"
