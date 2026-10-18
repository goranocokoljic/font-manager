<#
.SYNOPSIS
  Compares the core rasterizer (fm-core preview) with the Windows text renderer.

.DESCRIPTION
  For each sample, renders the same text at the same pixel size twice: with
  `cargo run -p fm-core --example render_preview` and with WPF (the Windows
  text stack: OpenType shaping, grayscale antialiasing, 96 DPI so 1 px = 1 DIP).
  Both images are cropped to their ink box (pixels at least 50% dark) and
  compared: ink box size, and the mean absolute difference of darkness over
  the union of the two boxes aligned at the top-left. Writes the PNGs and
  results.md next to this script.

  Windows only (WPF). Run from the repository root:
    pwsh -File docs/previews/compare-os.ps1
#>
param(
  [string]$Out = (Join-Path $PSScriptRoot 'samples')
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName PresentationCore, WindowsBase

$fonts = Join-Path (Get-Location) 'fixtures/fonts'
$samples = @(
  @{ name = 'robotoslab-48';  font = 'google-static/robotoslab/RobotoSlab-ExtraBold.ttf'; text = 'Hamburgefonstiv 0123'; size = 48 }
  @{ name = 'robotoslab-14';  font = 'google-static/robotoslab/RobotoSlab-ExtraBold.ttf'; text = 'Hamburgefonstiv 0123'; size = 14 }
  @{ name = 'asapsharp-36';   font = 'google-repo/asapsharp/AsapSharp[wdth,wght].ttf'; text = 'Quick brown fox AVATAR'; size = 36 }
  @{ name = 'asap-cff-36';    font = 'fontshare/asap/Asap-SemiBold.otf'; text = 'Typography 2026'; size = 36 }
  @{ name = 'armenian-36';    font = 'google-static/notosansarmenian/NotoSansArmenian-Medium.ttf'; text = 'Բարեւ աշխարհ'; size = 36 }
  @{ name = 'syriac-36';      font = 'google-static/notosanssyriaceastern/NotoSansSyriacEastern-Regular.ttf'; text = 'ܫܠܡܐ ܥܠܡܐ'; size = 36 }
)

New-Item -ItemType Directory -Force $Out | Out-Null

function Get-Darkness([System.Windows.Media.Imaging.BitmapSource]$src) {
  $bgra = New-Object System.Windows.Media.Imaging.FormatConvertedBitmap($src, [System.Windows.Media.PixelFormats]::Bgra32, $null, 0)
  $w = $bgra.PixelWidth; $h = $bgra.PixelHeight
  $px = New-Object byte[] ($w * $h * 4)
  $bgra.CopyPixels($px, $w * 4, 0)
  $dark = New-Object double[] ($w * $h)
  for ($i = 0; $i -lt $w * $h; $i++) {
    # White background: darkness = 1 - luminance of the composited pixel.
    $a = $px[$i * 4 + 3] / 255.0
    $lum = (0.2126 * $px[$i * 4 + 2] + 0.7152 * $px[$i * 4 + 1] + 0.0722 * $px[$i * 4]) / 255.0
    $dark[$i] = 1.0 - ($lum * $a + (1.0 - $a))
  }
  [pscustomobject]@{ W = $w; H = $h; D = $dark }
}

function Get-InkBox($img) {
  $x0 = $img.W; $y0 = $img.H; $x1 = -1; $y1 = -1
  for ($y = 0; $y -lt $img.H; $y++) {
    for ($x = 0; $x -lt $img.W; $x++) {
      if ($img.D[$y * $img.W + $x] -ge 0.5) {
        if ($x -lt $x0) { $x0 = $x }; if ($x -gt $x1) { $x1 = $x }
        if ($y -lt $y0) { $y0 = $y }; if ($y -gt $y1) { $y1 = $y }
      }
    }
  }
  [pscustomobject]@{ X = $x0; Y = $y0; W = $x1 - $x0 + 1; H = $y1 - $y0 + 1 }
}

function Read-Png([string]$path) {
  $stream = [System.IO.File]::OpenRead($path)
  try {
    $dec = [System.Windows.Media.Imaging.PngBitmapDecoder]::new($stream, 'PreservePixelFormat', 'OnLoad')
    $dec.Frames[0]
  } finally { $stream.Dispose() }
}

function Render-Os([string]$fontPath, [string]$text, [double]$size, [string]$png) {
  $glyphs = [System.Windows.Media.GlyphTypeface]::new([Uri]::new($fontPath))
  $family = $glyphs.Win32FamilyNames[[System.Globalization.CultureInfo]::GetCultureInfo('en-US')]
  $dir = [Uri]::new((Split-Path $fontPath) + [IO.Path]::DirectorySeparatorChar)
  $ff = [System.Windows.Media.FontFamily]::new($dir, "./#$family")
  $tf = [System.Windows.Media.Typeface]::new($ff, $glyphs.Style, $glyphs.Weight, $glyphs.Stretch)
  $ft = [System.Windows.Media.FormattedText]::new($text, [System.Globalization.CultureInfo]::GetCultureInfo('en-US'),
    'LeftToRight', $tf, $size, [System.Windows.Media.Brushes]::Black, 1.0)
  $w = [int][Math]::Ceiling($ft.WidthIncludingTrailingWhitespace + $size)
  $h = [int][Math]::Ceiling($ft.Height + $size)
  $dv = [System.Windows.Media.DrawingVisual]::new()
  $dc = $dv.RenderOpen()
  $dc.DrawRectangle([System.Windows.Media.Brushes]::White, $null, [System.Windows.Rect]::new(0, 0, $w, $h))
  $dc.DrawText($ft, [System.Windows.Point]::new($size / 2, $size / 2))
  $dc.Close()
  $rtb = [System.Windows.Media.Imaging.RenderTargetBitmap]::new($w, $h, 96, 96, [System.Windows.Media.PixelFormats]::Pbgra32)
  $rtb.Render($dv)
  $enc = [System.Windows.Media.Imaging.PngBitmapEncoder]::new()
  $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($rtb))
  $fs = [System.IO.File]::Create($png); try { $enc.Save($fs) } finally { $fs.Dispose() }
}

$rows = @()
foreach ($s in $samples) {
  $font = if ([IO.Path]::IsPathRooted($s.font)) { $s.font } else { Join-Path $fonts $s.font }
  $font = (Resolve-Path -LiteralPath $font).Path
  $corePng = Join-Path $Out "$($s.name)-core.png"
  $osPng = Join-Path $Out "$($s.name)-windows.png"
  cargo run -q -p fm-core --example render_preview -- $font $s.text $s.size $corePng | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "render_preview failed for $($s.name)" }
  Render-Os $font $s.text $s.size $osPng

  $core = Get-Darkness (Read-Png $corePng); $os = Get-Darkness (Read-Png $osPng)
  $cb = Get-InkBox $core; $ob = Get-InkBox $os
  $uw = [Math]::Max($cb.W, $ob.W); $uh = [Math]::Max($cb.H, $ob.H)
  $sum = 0.0
  for ($y = 0; $y -lt $uh; $y++) {
    for ($x = 0; $x -lt $uw; $x++) {
      $a = if ($x -lt $cb.W -and $y -lt $cb.H -and ($cb.X + $x) -lt $core.W -and ($cb.Y + $y) -lt $core.H) { $core.D[($cb.Y + $y) * $core.W + $cb.X + $x] } else { 0 }
      $b = if ($x -lt $ob.W -and $y -lt $ob.H -and ($ob.X + $x) -lt $os.W -and ($ob.Y + $y) -lt $os.H) { $os.D[($ob.Y + $y) * $os.W + $ob.X + $x] } else { 0 }
      $sum += [Math]::Abs($a - $b)
    }
  }
  # The core writes stored (uncompressed) PNGs; re-encode them losslessly so
  # the committed samples stay small.
  $enc = [System.Windows.Media.Imaging.PngBitmapEncoder]::new()
  $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create((Read-Png $corePng)))
  $ms = [System.IO.MemoryStream]::new(); $enc.Save($ms)
  [System.IO.File]::WriteAllBytes($corePng, $ms.ToArray())

  $rows += [pscustomobject]@{
    Sample = $s.name; Font = (Split-Path $font -Leaf); Size = $s.size
    CoreInk = "$($cb.W)x$($cb.H)"; WindowsInk = "$($ob.W)x$($ob.H)"
    DeltaW = $cb.W - $ob.W; DeltaH = $cb.H - $ob.H
    MeanAbsDiff = [Math]::Round($sum / ($uw * $uh), 4)
  }
}

$md = @('| Sample | Font | px | Core ink box | Windows ink box | Δw | Δh | Mean abs. darkness diff |',
        '|---|---|---|---|---|---|---|---|')
foreach ($r in $rows) {
  $md += "| $($r.Sample) | $($r.Font) | $($r.Size) | $($r.CoreInk) | $($r.WindowsInk) | $($r.DeltaW) | $($r.DeltaH) | $($r.MeanAbsDiff) |"
}
$md | Set-Content -Encoding utf8 (Join-Path $Out 'results.md')
$rows | Format-Table -AutoSize
