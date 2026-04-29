# Generates Stargate-themed PNG icons for kree.
# Run from repo root: pwsh -File scripts/generate-icons.ps1

Add-Type -AssemblyName System.Drawing

function New-StargateIcon {
    param(
        [int]$Size,
        [string]$OutPath,
        [bool]$Paused
    )

    $bmp = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    $cx = [int]($Size / 2)
    $cy = [int]($Size / 2)
    $rOuter = [int](($Size / 2) - 2)
    $ringWidth = [int]($Size * 0.16)
    $rInner = $rOuter - $ringWidth

    # Outer ring — Stargate naquadah (warm bronze/grey, desaturated when paused)
    $ringRect = New-Object System.Drawing.Rectangle (
        ($cx - $rOuter),
        ($cy - $rOuter),
        ($rOuter * 2),
        ($rOuter * 2)
    )
    if ($Paused) {
        $ringHi  = [System.Drawing.Color]::FromArgb(255, 130, 130, 138)
        $ringLo  = [System.Drawing.Color]::FromArgb(255, 55,  55,  62)
    } else {
        $ringHi  = [System.Drawing.Color]::FromArgb(255, 175, 155, 110)
        $ringLo  = [System.Drawing.Color]::FromArgb(255, 70,  58,  38)
    }
    $ringBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $ringRect, $ringHi, $ringLo, 45.0
    )
    $g.FillEllipse($ringBrush, $ringRect)

    # Inner event horizon — bright cyan-white center fading to dark teal
    $innerRect = New-Object System.Drawing.Rectangle (
        ($cx - $rInner),
        ($cy - $rInner),
        ($rInner * 2),
        ($rInner * 2)
    )
    if ($Paused) {
        $core = [System.Drawing.Color]::FromArgb(255, 195, 195, 200)
        $edge = [System.Drawing.Color]::FromArgb(255, 70,  70,  80)
    } else {
        $core = [System.Drawing.Color]::FromArgb(255, 220, 250, 255)
        $edge = [System.Drawing.Color]::FromArgb(255, 0,   80,  130)
    }
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddEllipse($innerRect)
    $pgBrush = New-Object System.Drawing.Drawing2D.PathGradientBrush($path)
    $pgBrush.CenterColor = $core
    $pgBrush.SurroundColors = @($edge)
    $g.FillEllipse($pgBrush, $innerRect)

    # Seven chevrons evenly around the ring (Stargate signature)
    $chevSize = [int]($Size * 0.10)
    if ($chevSize -lt 4) { $chevSize = 4 }
    if ($Paused) {
        $chevColor = [System.Drawing.Color]::FromArgb(255, 110, 110, 118)
    } else {
        $chevColor = [System.Drawing.Color]::FromArgb(255, 255, 195, 60)
    }
    $chevBrush = New-Object System.Drawing.SolidBrush($chevColor)
    $chevRadius = $rOuter - [int]($ringWidth / 2)
    for ($i = 0; $i -lt 7; $i++) {
        # Chevron #1 sits at top (12 o'clock); rest spread clockwise
        $angle = (-90.0 + ($i * (360.0 / 9.0))) * [Math]::PI / 180.0
        $px = $cx + ($chevRadius * [Math]::Cos($angle))
        $py = $cy + ($chevRadius * [Math]::Sin($angle))

        # Chevron triangle: tip points outward (away from gate center)
        $outDx = [Math]::Cos($angle)
        $outDy = [Math]::Sin($angle)
        $perpDx = -$outDy
        $perpDy = $outDx

        $tipX = $px + ($outDx * $chevSize * 0.6)
        $tipY = $py + ($outDy * $chevSize * 0.6)
        $b1X  = $px - ($outDx * $chevSize * 0.4) + ($perpDx * $chevSize * 0.5)
        $b1Y  = $py - ($outDy * $chevSize * 0.4) + ($perpDy * $chevSize * 0.5)
        $b2X  = $px - ($outDx * $chevSize * 0.4) - ($perpDx * $chevSize * 0.5)
        $b2Y  = $py - ($outDy * $chevSize * 0.4) - ($perpDy * $chevSize * 0.5)

        $points = @(
            (New-Object System.Drawing.PointF([float]$tipX, [float]$tipY)),
            (New-Object System.Drawing.PointF([float]$b1X,  [float]$b1Y)),
            (New-Object System.Drawing.PointF([float]$b2X,  [float]$b2Y))
        )
        $g.FillPolygon($chevBrush, [System.Drawing.PointF[]]$points)
    }

    # Top chevron (1) gets a bright accent ring to mark "active"
    if (-not $Paused) {
        $topAngle = -90.0 * [Math]::PI / 180.0
        $tx = $cx + ($chevRadius * [Math]::Cos($topAngle))
        $ty = $cy + ($chevRadius * [Math]::Sin($topAngle))
        $glowSize = [int]($chevSize * 1.3)
        $glowRect = New-Object System.Drawing.Rectangle (
            ([int]($tx - $glowSize / 2)),
            ([int]($ty - $glowSize / 2)),
            $glowSize, $glowSize
        )
        $glowPath = New-Object System.Drawing.Drawing2D.GraphicsPath
        $glowPath.AddEllipse($glowRect)
        $glowBrush = New-Object System.Drawing.Drawing2D.PathGradientBrush($glowPath)
        $glowBrush.CenterColor = [System.Drawing.Color]::FromArgb(180, 255, 230, 120)
        $glowBrush.SurroundColors = @([System.Drawing.Color]::FromArgb(0, 255, 230, 120))
        $g.FillEllipse($glowBrush, $glowRect)
    }

    $bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    $bmp.Dispose()
    Write-Host "wrote $OutPath"
}

$assets = Join-Path $PSScriptRoot "..\assets"
$assets = (Resolve-Path $assets).Path

New-StargateIcon -Size 64  -OutPath (Join-Path $assets "tray-icon.png")        -Paused $false
New-StargateIcon -Size 64  -OutPath (Join-Path $assets "tray-icon-paused.png") -Paused $true
New-StargateIcon -Size 256 -OutPath (Join-Path $assets "app-icon.png")         -Paused $false
