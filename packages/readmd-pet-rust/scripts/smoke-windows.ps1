param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$RendererRoot
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This smoke test requires Windows.' }
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class PetSmokeWin32 {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    public delegate bool EnumCallback(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumCallback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr window, int index);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window, uint message, IntPtr wp, IntPtr lp);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    public static IntPtr Find(uint target) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid == target && IsWindowVisible(window)) { found = window; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static Rect Bounds(IntPtr window) { Rect rect; if (!GetWindowRect(window, out rect)) throw new Exception("GetWindowRect failed"); return rect; }
}
'@
$previousDpi = [PetSmokeWin32]::SetThreadDpiAwarenessContext([IntPtr](-4))
$cursor = [PetSmokeWin32+Point]::new()
[void][PetSmokeWin32]::GetCursorPos([ref]$cursor)
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('readmd-pet-native-' + [guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory -Path $fixture)
$bridge = Join-Path $fixture 'state.json'
$snapshot = @{ format_version=1; generation=1; renderer='hermes-sprite'; visible=$true; bounds=@{x=200;y=100;width=320;height=420}; info=@{character='mochi'} }
function Publish-Snapshot { $snapshot | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath $bridge -Encoding utf8NoBOM }
function Require($condition, $message) { if (-not $condition) { throw $message } }
Publish-Snapshot
$names = @('READMD_PET_BRIDGE_FILE','READMD_PET_RENDERER_ROOT','READMD_DATA_DIR','READMD_PARENT_PID','READMD_PARENT_PIPE_HANDLE')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$hostProcess = $null
$window = [IntPtr]::Zero
try {
    $env:READMD_PET_BRIDGE_FILE = $bridge
    $env:READMD_PET_RENDERER_ROOT = (Resolve-Path -LiteralPath $RendererRoot).Path
    $env:READMD_DATA_DIR = $fixture
    $env:READMD_PARENT_PID = "$PID"
    [Environment]::SetEnvironmentVariable('READMD_PARENT_PIPE_HANDLE', $null, 'Process')
    $hostProcess = Start-Process -FilePath (Resolve-Path -LiteralPath $Executable).Path -WorkingDirectory $fixture -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $fixture 'stderr.log') -RedirectStandardOutput (Join-Path $fixture 'stdout.log')
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $ready = $false
    while ([DateTime]::UtcNow -lt $deadline) {
        Require (-not $hostProcess.HasExited) "Isolated host exited; see $fixture/stderr.log"
        try {
            $health = Get-Content -LiteralPath "$bridge.rust.health.json" -Raw | ConvertFrom-Json
            if ($health.state -eq 'ready' -and $health.pid -eq $hostProcess.Id) { $ready = $true; break }
        } catch { }
        Start-Sleep -Milliseconds 100
    }
    Require $ready "Isolated host did not become ready; see $fixture"
    $window = [PetSmokeWin32]::Find($hostProcess.Id)
    Require ($window -ne [IntPtr]::Zero) 'No visible native overlay for the isolated host'
    $before = [PetSmokeWin32]::Bounds($window)
    $width = $before.Right - $before.Left
    $height = $before.Bottom - $before.Top
    $x = $before.Left + [int]($width * 0.5)
    $y = $before.Top + [int]($height * (305.0 / 420.0))
    [void][PetSmokeWin32]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 180
    Require (([PetSmokeWin32]::GetWindowLongPtrW($window, -20).ToInt64() -band 0x20) -eq 0) 'Visible pet remained click-through under the pointer'
    [PetSmokeWin32]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    [void][PetSmokeWin32]::SetCursorPos($x + 3, $y)
    Start-Sleep -Milliseconds 100
    $smallMove = [PetSmokeWin32]::Bounds($window)
    Require ($smallMove.Left -eq $before.Left -and $smallMove.Top -eq $before.Top) 'Below-threshold motion moved the window'
    [void][PetSmokeWin32]::SetCursorPos($x + 18, $y + 8)
    Start-Sleep -Milliseconds 180
    for ($step = 1; $step -le 10; $step++) {
        [void][PetSmokeWin32]::SetCursorPos($x + 18 + $step * 12, $y + 8 + $step * 6)
        Start-Sleep -Milliseconds 35
    }
    [PetSmokeWin32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
    $after = [PetSmokeWin32]::Bounds($window)
    Require (($after.Left - $before.Left) -ge 100 -and ($after.Top - $before.Top) -ge 40) "Native drag did not follow the held-pointer gesture: delta $($after.Left-$before.Left),$($after.Top-$before.Top); fixture $fixture"
    Require (($after.Right - $after.Left) -eq $width -and ($after.Bottom - $after.Top) -eq $height) 'Drag changed the window size'
    $commands = @(Get-ChildItem -LiteralPath "$bridge.commands" -File | ForEach-Object { (Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json).command })
    $boundsCommand = @($commands | Where-Object type -eq 'bounds')[-1]
    Require ($null -ne $boundsCommand) 'Drag end did not publish durable bounds'
    $snapshot.generation = 2
    Publish-Snapshot
    Start-Sleep -Milliseconds 250
    $guarded = [PetSmokeWin32]::Bounds($window)
    Require ($guarded.Left -eq $after.Left -and $guarded.Top -eq $after.Top) 'Queued old bridge bounds snapped the overlay back'
    $snapshot.generation = 3
    $snapshot.bounds = $boundsCommand.bounds
    Publish-Snapshot
    Start-Sleep -Milliseconds 250
    $acknowledged = [PetSmokeWin32]::Bounds($window)
    Require ($acknowledged.Left -eq $after.Left -and $acknowledged.Top -eq $after.Top) 'Acknowledging final drag bounds moved the overlay'
    [void][PetSmokeWin32]::SetCursorPos($after.Left + 2, $after.Top + 2)
    Start-Sleep -Milliseconds 180
    Require (([PetSmokeWin32]::GetWindowLongPtrW($window, -20).ToInt64() -band 0x20) -ne 0) 'Transparent margin did not restore click-through'
    Write-Output "Native drag PASS: physical delta $($after.Left-$before.Left),$($after.Top-$before.Top); durable bounds, stale snapshot guard, acknowledgement and transparent-margin click-through. Fixture: $fixture"
} finally {
    [PetSmokeWin32]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    [void][PetSmokeWin32]::SetCursorPos($cursor.X, $cursor.Y)
    [void][PetSmokeWin32]::SetThreadDpiAwarenessContext($previousDpi)
    if ($null -ne $hostProcess -and -not $hostProcess.HasExited) {
        if ($window -ne [IntPtr]::Zero) { [void][PetSmokeWin32]::PostMessageW($window, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) }
        if (-not $hostProcess.WaitForExit(5000)) { Stop-Process -Id $hostProcess.Id }
    }
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
