param(
    [Parameter(Mandatory=$true)][string]$OutputPath,
    [string]$BaseSha = '2f55ae825848e183a7840032782e37cbc54e641d',
    [string]$Zig = 'W:\devin_folder\tools\zig-x86_64-windows-0.15.2\zig.exe'
)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $OutputPath) { throw 'Use a new output path; recorded preflight metadata is immutable.' }
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class DisplayPreflight {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct MonitorInfo {
        public int Size; public Rect Monitor, Work; public uint Flags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string Device;
    }
    [StructLayout(LayoutKind.Explicit, CharSet=CharSet.Unicode, Size=220)] public struct Mode {
        [FieldOffset(0), MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string Device;
        [FieldOffset(68)] public ushort Size;
        [FieldOffset(70)] public ushort Extra;
        [FieldOffset(172)] public uint Width;
        [FieldOffset(176)] public uint Height;
        [FieldOffset(184)] public uint Refresh;
    }
    public sealed class Row {
        public string Device; public bool Primary;
        public int Left, Top, Right, Bottom, ScalePercent;
        public uint PixelWidth, PixelHeight, RefreshHz;
        public int ScaleHresult;
    }
    public delegate bool Callback(IntPtr monitor, IntPtr dc, ref Rect rect, IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumDisplayMonitors(IntPtr dc, IntPtr clip, Callback callback, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool GetMonitorInfoW(IntPtr monitor, ref MonitorInfo info);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern bool EnumDisplaySettingsW(string device, int index, ref Mode mode);
    [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr value);
    [DllImport("shcore.dll")] static extern int GetScaleFactorForMonitor(IntPtr monitor, out int scale);
    public static Row[] Read() {
        IntPtr previous = SetThreadDpiAwarenessContext(new IntPtr(-4));
        if (previous == IntPtr.Zero) throw new InvalidOperationException("Cannot set per-monitor-v2 context.");
        var rows = new List<Row>();
        Exception failure = null;
        Callback callback = delegate(IntPtr monitor, IntPtr dc, ref Rect rect, IntPtr data) {
            var info = new MonitorInfo(); info.Size = Marshal.SizeOf(typeof(MonitorInfo));
            var mode = new Mode(); mode.Size = (ushort)Marshal.SizeOf(typeof(Mode));
            if (!GetMonitorInfoW(monitor, ref info) || !EnumDisplaySettingsW(info.Device, -1, ref mode)) {
                failure = new InvalidOperationException("Cannot read active display mode."); return false;
            }
            int scale; int hr = GetScaleFactorForMonitor(monitor, out scale);
            rows.Add(new Row { Device=info.Device, Primary=(info.Flags & 1)!=0,
                Left=info.Monitor.Left, Top=info.Monitor.Top, Right=info.Monitor.Right, Bottom=info.Monitor.Bottom,
                PixelWidth=mode.Width, PixelHeight=mode.Height, RefreshHz=mode.Refresh,
                ScalePercent=scale, ScaleHresult=hr });
            return true;
        };
        try {
            bool success = EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, callback, IntPtr.Zero);
            if (failure != null) throw failure;
            if (!success) throw new InvalidOperationException("Cannot enumerate active displays.");
            return rows.ToArray();
        } finally { SetThreadDpiAwarenessContext(previous); GC.KeepAlive(callback); }
    }
}
'@
$os = Get-CimInstance Win32_OperatingSystem
$machine = Get-CimInstance Win32_ComputerSystem
$cpus = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
$build = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$languages = @()
foreach ($language in (Get-WinUserLanguageList)) {
    $languages += [ordered]@{ language_tag=$language.LanguageTag; input_method_tips=@($language.InputMethodTips) }
}
$drivers = @()
foreach ($driver in (Get-CimInstance Win32_PnPSignedDriver -Filter "DeviceClass = 'DISPLAY'")) {
    $drivers += [ordered]@{ name=$driver.DeviceName; version=$driver.DriverVersion; provider=$driver.DriverProviderName; signed=$driver.IsSigned; signer=$driver.Signer }
}
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
$vs = if (Test-Path $vswhere) { & $vswhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json | ConvertFrom-Json } else { @() }
$vsTools = @()
foreach ($instance in $vs) {
    $vsTools += [ordered]@{ name=$instance.DisplayName; version=$instance.InstallationVersion; path=$instance.InstallationPath }
}
$toolchains = [ordered]@{}
foreach ($name in @('rustc','cargo','go','python','git')) {
    $command = @(Get-Command $name -CommandType Application -ErrorAction Stop)[0]
    $arguments = @(if ($name -eq 'go') { 'version' } else { '--version' })
    $version = & $command.Source @arguments 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "$name version failed." }
    $toolchains[$name] = [ordered]@{path=$command.Source; version=$version.Trim()}
}
$zigVersion = (& $Zig version | Out-String).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Zig version failed.' }
$toolchains.zig = [ordered]@{path=$Zig; version=$zigVersion}
$record = [ordered]@{
    schema = 'mascot-windows-preflight-1'
    captured_utc = [DateTime]::UtcNow.ToString('o')
    base_sha = $BaseSha
    os = [ordered]@{ caption=$os.Caption; version=$os.Version; build=$os.BuildNumber; update_build_revision=$build.UBR; architecture=$os.OSArchitecture; last_boot_utc=$os.LastBootUpTime.ToUniversalTime().ToString('o') }
    cpu = $cpus
    physical_ram_bytes = [uint64]$machine.TotalPhysicalMemory
    power_scheme = ((& powercfg /getactivescheme) | Out-String).Trim()
    displays = @([DisplayPreflight]::Read())
    display_mode_source = 'EnumDisplayMonitors/GetMonitorInfoW/EnumDisplaySettingsW, per-monitor-v2 thread context; integral refresh Hz'
    display_scale_source = 'GetScaleFactorForMonitor'
    display_drivers = $drivers
    input_languages = $languages
    toolchains = $toolchains
    visual_studio = $vsTools
    windows_sdk = @(Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\Include' -Directory | Select-Object -ExpandProperty Name)
    no_automatic_reboot = $true
}
$parent = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($OutputPath))
[void][IO.Directory]::CreateDirectory($parent)
$record | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $OutputPath -Encoding UTF8
Write-Output $OutputPath
