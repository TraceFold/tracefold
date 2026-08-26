# SPDX-License-Identifier: Apache-2.0
# Open the demo in a real window, read what the window says about itself, and photograph it.
#
# Headless is not the window. A page can lay out correctly in a headless dump and still
# arrive on a screen with two things printed over each other, or with a mark at the SVG
# format's default size, or with nothing painted behind it at all. So the shell is required
# to have been on a screen at least once, and the evidence is a picture a person looked at.
#
# The window's own title carries the check count, which is how a number computed inside the
# page can be read from outside it without a debugger port. The picture is for the eye; the
# title is for the record.
#
#   powershell -File tools/real_window.ps1 -Url http://127.0.0.1:8788/ -Out <dir>

param(
  [string]$Url = 'http://127.0.0.1:8788/',
  [string]$Out = "$env:TEMP",
  [string]$Name = 'real_window',
  [int]$Wait = 6,
  # Which window to photograph, as a title pattern.
  #
  # This used to be the literal 'glovrex*', written into the filter below, and it picks
  # the FIRST chrome window whose title matches -- which on a machine with any other
  # Glovrex page open is not necessarily this script's own window. It happened: a capture
  # run during retrofit r4 photographed an unrelated status page that another session had
  # open, and wrote it over a committed record shot (restored from git; no evidence was
  # lost, but a picture of the wrong thing had a correct-looking filename for a minute).
  # req/97 had already recorded the other half of this blind spot from the opposite
  # direction -- the same pattern cannot see a window titled 'Studio - Glovrex_Studio'.
  #
  # The default is unchanged, so every existing caller behaves exactly as before. A caller
  # that knows its own page's title (they all do -- each page writes its own check count
  # into it) can now say so and be sure of what it photographed.
  [string]$TitleLike = 'glovrex*'
)

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

$chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
if (-not (Test-Path $chrome)) { Write-Error 'chrome is not where this expects it'; exit 1 }

$profile = Join-Path $env:TEMP ("gx-shell-window-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
$arguments = @(
  "--app=$Url",
  '--new-window',
  '--window-size=1440,900',
  '--window-position=40,40',
  "--user-data-dir=$profile",
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-features=Translate,MediaRouter'
)

# Park the cursor away from where the window will open. A pointer resting on a sash when
# the window takes focus is a real drag as far as the shell is concerned -- it moved the
# digest by a pixel of dock width in an earlier capture, and the strip said so, which is
# correct behaviour and a useless photograph.
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(2, 2)

$process = Start-Process -FilePath $chrome -ArgumentList $arguments -PassThru
Start-Sleep -Seconds $Wait
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(2, 2)

$window = Get-Process -Name chrome -ErrorAction SilentlyContinue |
  Where-Object { $_.MainWindowTitle -like $TitleLike } |
  Select-Object -First 1

if ($null -eq $window) {
  Write-Output ("NO WINDOW: no chrome window has a title like " + $TitleLike)
  Get-Process -Name chrome -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle } | ForEach-Object { Write-Output ("  saw: " + $_.MainWindowTitle) }
} else {
  Write-Output ("TITLE: " + $window.MainWindowTitle)
}

Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left, Top, Right, Bottom; }
public class Win {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
}
"@

# Owner eye-judgment correction (2026-08-25): captures were bleeding a sliver of
# whatever sat behind the window at its edges. GetWindowRect on modern Windows
# includes several pixels of invisible DWM resize/drop-shadow border outside the
# window's actual painted content -- CopyFromScreen against that rect copies a few
# pixels of desktop/background window past each edge along with the real picture.
# DWMWA_EXTENDED_FRAME_BOUNDS (attribute 9) is the documented way to ask DWM for
# the true visible bounds instead of the legacy window-manager rect; it is used
# here when it succeeds, with GetWindowRect kept as the fallback so a machine
# where the DWM call fails (or window is off-DWM) still produces a picture rather
# than none at all.
if ($null -ne $window) {
  [Win]::ShowWindow($window.MainWindowHandle, 5) | Out-Null
  [Win]::BringWindowToTop($window.MainWindowHandle) | Out-Null
  [Win]::SetForegroundWindow($window.MainWindowHandle) | Out-Null
  Start-Sleep -Milliseconds 900
  # Whether the window we are about to photograph is actually the one on top.
  #
  # SetForegroundWindow is a request, not a command: Windows refuses it when the calling
  # process does not own the foreground, which is exactly the case when this script is
  # driven from a terminal that has focus. It returns and the window stays behind -- and
  # CopyFromScreen then copies whatever IS in front, at the right window's coordinates,
  # into a file with the right window's name. That happened during retrofit r4: four
  # captures came back as pictures of a terminal, and nothing in the output said so. An
  # instrument that photographs the wrong thing and reports success is fail-open, which
  # is the failure class this whole project is built against.
  #
  # So the fact is measured and printed rather than assumed, and the capture below picks
  # its method from it.
  $isForeground = [Win]::GetForegroundWindow() -eq $window.MainWindowHandle
  Write-Output ("FOREGROUND: " + $isForeground)
  $rect = New-Object RECT
  $dwmRect = New-Object RECT
  $dwmOk = [Win]::DwmGetWindowAttribute($window.MainWindowHandle, 9, [ref]$dwmRect, [System.Runtime.InteropServices.Marshal]::SizeOf([type][RECT])) -eq 0
  if ($dwmOk -and ($dwmRect.Right -gt $dwmRect.Left) -and ($dwmRect.Bottom -gt $dwmRect.Top)) {
    $rect = $dwmRect
    Write-Output "RECT-SOURCE: DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)"
  } else {
    [Win]::GetWindowRect($window.MainWindowHandle, [ref]$rect) | Out-Null
    Write-Output "RECT-SOURCE: GetWindowRect (DWM call unavailable, fallback)"
  }
  $w = $rect.Right - $rect.Left
  $h = $rect.Bottom - $rect.Top
  Write-Output ("RECT: {0},{1} {2}x{3}" -f $rect.Left, $rect.Top, $w, $h)
  if ($w -gt 0 -and $h -gt 0) {
    $bitmap = New-Object System.Drawing.Bitmap $w, $h
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    if ($isForeground) {
      # The window is on top, so the screen IS the window. This is the original method
      # and it stays the preferred one: it photographs what a person would actually see,
      # compositing and all.
      $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
      Write-Output "CAPTURE: CopyFromScreen (window is on top)"
    } else {
      # The window is behind something. PrintWindow asks the window to draw itself into
      # a device context, so what comes back is this window's own content regardless of
      # what is stacked over it. Flag 2 is PW_RENDERFULLCONTENT, which is what makes it
      # work for a hardware-composited surface like a browser's; without it a Chrome
      # window renders blank.
      #
      # It is the fallback and not the default because it captures the window's own
      # drawing rather than the screen, so it can differ from what a person sees in
      # exactly the ways compositing differs -- which is worth knowing about rather than
      # hiding. The line printed above says which of the two produced the file.
      $dc = $graphics.GetHdc()
      $ok = [Win]::PrintWindow($window.MainWindowHandle, $dc, 2)
      $graphics.ReleaseHdc($dc)
      Write-Output ("CAPTURE: PrintWindow PW_RENDERFULLCONTENT (window is behind something) ok=" + $ok)
    }
    $path = Join-Path $Out ($Name + '.png')
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    Write-Output ("SHOT: " + $path)
  }
}

Start-Sleep -Seconds 1
Get-Process -Name chrome -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $chrome -and $_.MainWindowTitle -like 'glovrex*' } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
if ($null -ne $process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
Remove-Item -Recurse -Force $profile -ErrorAction SilentlyContinue
