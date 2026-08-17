# PonyClean dev helper: screenshot + synthetic mouse ops (Windows, FullLanguage pwsh)
param(
  [string]$Path = ".dev\shot.png",
  [int]$ClickX = -1,
  [int]$ClickY = -1,
  [int]$MoveX = -1,
  [int]$MoveY = -1
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NativeInput {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT pt);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
}
"@

if ($MoveX -ge 0 -and $MoveY -ge 0) {
  [NativeInput]::SetCursorPos($MoveX, $MoveY) | Out-Null
  Start-Sleep -Milliseconds 120
}
if ($ClickX -ge 0 -and $ClickY -ge 0) {
  [NativeInput]::SetCursorPos($ClickX, $ClickY) | Out-Null
  Start-Sleep -Milliseconds 150
  [NativeInput]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)  # LEFTDOWN
  Start-Sleep -Milliseconds 60
  [NativeInput]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)  # LEFTUP
  Start-Sleep -Milliseconds 250
}

$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
$g.Dispose()
$out = Join-Path (Get-Location) $Path
$dir = Split-Path $out -Parent
if (!(Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
$pt = New-Object NativeInput+POINT
[NativeInput]::GetCursorPos([ref]$pt) | Out-Null
Write-Output "saved: $out ($($bounds.Width)x$($bounds.Height)) cursor=$($pt.X),$($pt.Y)"
