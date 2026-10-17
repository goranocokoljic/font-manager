# E6.1 activation spike, Windows. Throwaway: it proves what the real
# FontActivator (E6.2+) can rely on; see docs/adr/0008-font-activation.md.
#
#   pwsh -File windows.ps1                 # run every scenario, print a results table
#   pwsh -File windows.ps1 -Mode check -Family Diplomata
#
# Every step that registers, removes or looks runs in its OWN pwsh process, so
# "a second process can see it" is what is measured, never the registering one.
[CmdletBinding()]
param(
    [ValidateSet('all', 'check', 'watch', 'add', 'remove', 'add-and-wait', 'reg-set', 'reg-del')]
    [string]$Mode = 'all',
    [string]$Family,
    [string]$Path,
    [string]$Value,
    [string]$Ready,
    [int]$Seconds = 20
)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class Gdi {
    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)] public static extern int AddFontResourceExW(string name, uint fl, IntPtr res);
    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)] public static extern bool RemoveFontResourceExW(string name, uint fl, IntPtr res);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageTimeoutW(IntPtr hWnd, uint msg, IntPtr w, IntPtr l, uint flags, uint timeout, out IntPtr result);
    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)] static extern IntPtr CreateFontW(int h, int w, int esc, int ori, int weight, uint italic, uint underline, uint strike, uint charset, uint outPrec, uint clipPrec, uint quality, uint pitch, string face);
    [DllImport("gdi32.dll")] static extern IntPtr CreateCompatibleDC(IntPtr dc);
    [DllImport("gdi32.dll")] static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr obj);
    [DllImport("gdi32.dll")] static extern bool DeleteDC(IntPtr dc);
    [DllImport("gdi32.dll", CharSet = CharSet.Unicode)] static extern int GetTextFaceW(IntPtr dc, int n, System.Text.StringBuilder face);

    public static void BroadcastFontChange() {
        IntPtr r; // HWND_BROADCAST, WM_FONTCHANGE, SMTO_ABORTIFHUNG
        SendMessageTimeoutW((IntPtr)0xffff, 0x001D, IntPtr.Zero, IntPtr.Zero, 0x0002, 1000, out r);
    }

    // GDI substitutes another face when the name is unknown; the selected face
    // name tells whether the font itself was found.
    public static bool Has(string family) {
        IntPtr dc = CreateCompatibleDC(IntPtr.Zero);
        IntPtr f = CreateFontW(-24, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 0, 0, family);
        IntPtr old = SelectObject(dc, f);
        var sb = new System.Text.StringBuilder(64);
        GetTextFaceW(dc, 64, sb);
        SelectObject(dc, old); DeleteObject(f); DeleteDC(dc);
        return string.Equals(sb.ToString(), family, StringComparison.OrdinalIgnoreCase);
    }
}

// DirectWrite is what WebView2, Office and most current apps render with.
[ComImport, Guid("b859ee5a-d838-4b5b-a2e8-1adc7d93db48"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IDWriteFactory {
    [PreserveSig] int GetSystemFontCollection(out IDWriteFontCollection collection, bool checkForUpdates);
}
[ComImport, Guid("a84cee02-3eea-4eee-a827-87c1a02a0fcc"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IDWriteFontCollection {
    [PreserveSig] uint GetFontFamilyCount();
    [PreserveSig] int GetFontFamily(uint index, out IntPtr family);
    [PreserveSig] int FindFamilyName([MarshalAs(UnmanagedType.LPWStr)] string name, out uint index, out bool exists);
}
public static class DWrite {
    [DllImport("dwrite.dll")] static extern int DWriteCreateFactory(int type, ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object factory);
    public static bool Has(string family) {
        Guid iid = typeof(IDWriteFactory).GUID;
        object o; Marshal.ThrowExceptionForHR(DWriteCreateFactory(1 /* isolated: no shared cache from earlier calls */, ref iid, out o));
        IDWriteFontCollection c;
        Marshal.ThrowExceptionForHR(((IDWriteFactory)o).GetSystemFontCollection(out c, true));
        uint i; bool exists;
        Marshal.ThrowExceptionForHR(c.FindFamilyName(family, out i, out exists));
        return exists;
    }
}
'@

$RegKey = 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts'

function Look([string]$f) { [ordered]@{ gdi = [Gdi]::Has($f); dwrite = [DWrite]::Has($f) } }

switch ($Mode) {
    'check' { Look $Family | ConvertTo-Json -Compress; return }
    'watch' {
        # An app that was already running before the font arrived.
        Set-Content $Ready 'up'
        $sw = [Diagnostics.Stopwatch]::StartNew(); $seen = [ordered]@{ gdi = $null; dwrite = $null }
        while ($sw.Elapsed.TotalSeconds -lt $Seconds -and ($null -in $seen.Values)) {
            $l = Look $Family
            foreach ($k in 'gdi', 'dwrite') { if ($l[$k] -and $null -eq $seen[$k]) { $seen[$k] = $sw.ElapsedMilliseconds } }
            Start-Sleep -Milliseconds 250
        }
        $seen | ConvertTo-Json -Compress; return
    }
    'add' { $n = [Gdi]::AddFontResourceExW($Path, 0, [IntPtr]::Zero); [Gdi]::BroadcastFontChange(); "$n"; return }
    'remove' { $ok = [Gdi]::RemoveFontResourceExW($Path, 0, [IntPtr]::Zero); [Gdi]::BroadcastFontChange(); "$ok"; return }
    'add-and-wait' {
        # Registers, then stays alive until $Ready is deleted: lets the caller
        # look while the registering process lives and after it exits.
        $n = [Gdi]::AddFontResourceExW($Path, 0, [IntPtr]::Zero); [Gdi]::BroadcastFontChange()
        Set-Content $Ready "$n"
        while (Test-Path $Ready) { Start-Sleep -Milliseconds 200 }
        return
    }
    'reg-set' { if (-not (Test-Path $RegKey)) { New-Item $RegKey -Force | Out-Null }; New-ItemProperty $RegKey -Name $Value -Value $Path -PropertyType String -Force | Out-Null; [Gdi]::BroadcastFontChange(); return }
    'reg-del' { Remove-ItemProperty $RegKey -Name $Value -ErrorAction SilentlyContinue; [Gdi]::BroadcastFontChange(); return }
}

# ---- orchestrator ------------------------------------------------------------
$root = (Resolve-Path "$PSScriptRoot/../..").Path
# Two fonts no CI image ships, so "before" is a real negative.
$session = @{ family = 'Diplomata'; path = (Join-Path $root 'fixtures/fonts/google-repo/diplomata/Diplomata-Regular.ttf') }
$user = @{ family = 'Emblema One'; path = (Join-Path $root 'fixtures/fonts/google-repo/emblemaone/EmblemaOne-Regular.ttf') }
foreach ($f in $session, $user) { if (-not (Test-Path $f.path)) { throw "missing fixture $($f.path)" } }

$rows = [System.Collections.Generic.List[object]]::new()
function Sub([string[]]$a) { (& pwsh -NoProfile -File $PSCommandPath @a | Out-String).Trim() }
function Row([string]$step, [string]$fam) {
    $r = Sub @('-Mode', 'check', '-Family', $fam) | ConvertFrom-Json
    $rows.Add([pscustomobject]@{ step = $step; gdi = $r.gdi; dwrite = $r.dwrite })
    Write-Host ("{0,-62} gdi={1,-5} dwrite={2}" -f $step, $r.gdi, $r.dwrite)
}
function Watcher([string]$fam) {
    $ready = Join-Path ([IO.Path]::GetTempPath()) "e61-watch-$([guid]::NewGuid()).txt"
    $j = Start-Job { param($s, $f, $r) pwsh -NoProfile -File $s -Mode watch -Family $f -Ready $r -Seconds 20 } -ArgumentList $PSCommandPath, $fam, $ready
    while (-not (Test-Path $ready)) { Start-Sleep -Milliseconds 200 }
    $j
}
function WatchRow([string]$step, $j) {
    $w = (Receive-Job $j -Wait -AutoRemoveJob | Out-String).Trim() | ConvertFrom-Json
    $rows.Add([pscustomobject]@{ step = $step; gdi = $w.gdi ?? 'not seen'; dwrite = $w.dwrite ?? 'not seen' })
    Write-Host ("{0,-62} gdi={1,-5} dwrite={2}  (ms after start)" -f $step, $w.gdi, $w.dwrite)
}

$os = Get-CimInstance Win32_OperatingSystem
Write-Host "== $($os.Caption) build $($os.BuildNumber)"

# Session: AddFontResourceEx without FR_PRIVATE + WM_FONTCHANGE.
Row 'session: before' $session.family
$j = Watcher $session.family
$ready = Join-Path ([IO.Path]::GetTempPath()) "e61-add-$([guid]::NewGuid()).txt"
$reg = Start-Process pwsh -ArgumentList '-NoProfile', '-File', $PSCommandPath, '-Mode', 'add-and-wait', '-Path', $session.path, '-Ready', $ready -PassThru -WindowStyle Hidden
while (-not (Test-Path $ready)) { Start-Sleep -Milliseconds 200 }
Write-Host "   AddFontResourceEx returned $(Get-Content $ready)"
Row 'session: registrar alive, new process' $session.family
WatchRow 'session: process already running before registration' $j
Remove-Item $ready; $reg.WaitForExit()
Start-Sleep 1
Row 'session: registrar exited, new process' $session.family
Sub @('-Mode', 'remove', '-Path', $session.path) | Out-Null
Row 'session: after RemoveFontResourceEx (other process)' $session.family

# Permanent, per-user: HKCU Fonts value pointing at the file in place.
$val = "$($user.family) (TrueType)"
Row 'user: before' $user.family
$j = Watcher $user.family
Sub @('-Mode', 'reg-set', '-Path', $user.path, '-Value', $val) | Out-Null
Row 'user: HKCU value only, new process' $user.family
WatchRow 'user: HKCU value only, process already running' $j
Sub @('-Mode', 'add', '-Path', $user.path) | Out-Null
Row 'user: HKCU value + AddFontResourceEx, new process' $user.family
Sub @('-Mode', 'remove', '-Path', $user.path) | Out-Null
Row 'user: HKCU value, AddFontResource removed, new process' $user.family
Sub @('-Mode', 'reg-del', '-Value', $val) | Out-Null
Row 'user: HKCU value deleted, new process' $user.family

$md = @("### Windows: $($os.Caption) build $($os.BuildNumber)", '', '| step | GDI | DirectWrite |', '|---|---|---|')
$md += $rows | ForEach-Object { "| $($_.step) | $($_.gdi) | $($_.dwrite) |" }
$md -join "`n" | Tee-Object -FilePath (Join-Path $PSScriptRoot "results-windows-$($os.BuildNumber).md") | Out-Null
if ($env:GITHUB_STEP_SUMMARY) { ($md -join "`n") + "`n" | Add-Content $env:GITHUB_STEP_SUMMARY }
