Add-Type -AssemblyName System.Drawing
$width = 1280
$height = 820
$bitmap = [Drawing.Bitmap]::new($width, $height)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.Clear([Drawing.Color]::FromArgb(14, 20, 40))
$background = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.Point]::new(0, 0), [Drawing.Point]::new($width, $height), [Drawing.Color]::FromArgb(14, 20, 40), [Drawing.Color]::FromArgb(32, 43, 93))
$graphics.FillRectangle($background, 0, 0, $width, $height)
$panel = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(23, 30, 55))
$border = [Drawing.Pen]::new([Drawing.Color]::FromArgb(58, 71, 119), 2)
$graphics.FillRectangle($panel, 260, 82, 760, 650)
$graphics.DrawRectangle($border, 260, 82, 760, 650)
$fontTitle = [Drawing.Font]::new('Microsoft YaHei UI', 30, [Drawing.FontStyle]::Bold)
$fontBody = [Drawing.Font]::new('Microsoft YaHei UI', 14)
$fontSmall = [Drawing.Font]::new('Microsoft YaHei UI', 12)
$fontStrong = [Drawing.Font]::new('Microsoft YaHei UI', 15, [Drawing.FontStyle]::Bold)
$muted = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(156, 168, 208))
$text = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(244, 245, 255))
$accent = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(189, 180, 255))
$graphics.DrawString('‹  返回设置', $fontBody, $muted, 62, 48)
$graphics.FillEllipse([Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(41, 46, 91)), 592, 122, 96, 96)
$iconPath = Join-Path $PSScriptRoot '..\public\nightowl-icon.png'
$icon = [Drawing.Image]::FromFile((Resolve-Path $iconPath))
$graphics.DrawImage($icon, 616, 146, 48, 48)
$icon.Dispose()
$graphics.DrawString('NightOwl Timer', $fontTitle, $text, 438, 245)
$graphics.DrawString('陪你把电脑安静地交给夜晚。', $fontBody, $muted, 505, 298)
$rows = @(@('版本', 'v1.0.1', $true), @('运行平台', 'windows', $true), @('代码仓库', 'https://github.com/Zverly/NightOwlTimer  ↗', $false), @('开源协议', 'MIT License · © 2026 Zverly', $true), @('本地数据', '5 条历史记录 · 应用目录\\data\\nightowl-data.json  ↗', $false))
$y = 342
foreach ($row in $rows) {
  $graphics.DrawString($row[0], $fontBody, $muted, 316, $y)
  $graphics.DrawString($row[1], $row[2] ? $fontStrong : $fontSmall, $row[2] ? $text : $accent, 690, $y)
  if ($y -lt 606) { $graphics.DrawLine([Drawing.Pen]::new([Drawing.Color]::FromArgb(48, 59, 103), 1), 316, $y + 38, 958, $y + 38) }
  $y += 64
}
$output = Join-Path $PSScriptRoot '..\docs\images\nightowl-about.png'
$bitmap.Save($output, [Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose(); $bitmap.Dispose()
