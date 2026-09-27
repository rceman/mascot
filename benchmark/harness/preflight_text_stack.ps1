param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new evidence directory.' }
[void][IO.Directory]::CreateDirectory($OutputDirectory)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Windows.Forms;
public static class NativeTextProbe {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr LoadLibraryExW(string name, IntPtr file, uint flags);
    [DllImport("kernel32.dll")] public static extern bool FreeLibrary(IntPtr module);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateWindowExW(uint ex, string cls, string name, uint style, int x, int y, int w, int h, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessageW(IntPtr window, uint msg, IntPtr wp, IntPtr lp);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SetWindowTextW(IntPtr window, string text);
    [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetFocus();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("gdi32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr CreateFontW(int h, int w, int e, int o, int weight, uint italic, uint underline, uint strike, uint charset, uint outp, uint clip, uint quality, uint pitch, string face);
    [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr value);
    [StructLayout(LayoutKind.Sequential)] public struct Range { public int Start, End; }
    [DllImport("user32.dll", EntryPoint="SendMessageW")] static extern IntPtr SendRange(IntPtr window, uint msg, IntPtr wp, ref Range range);
    public static Range Selection(IntPtr window) { var range = new Range(); SendRange(window, 0x434, IntPtr.Zero, ref range); return range; }
    [StructLayout(LayoutKind.Sequential)] public struct Input { public uint Type; public InputUnion Value; }
    [StructLayout(LayoutKind.Explicit)] public struct InputUnion {
        [FieldOffset(0)] public Keyboard Keyboard;
        [FieldOffset(0)] public Mouse Mouse;
    }
    [StructLayout(LayoutKind.Sequential)] public struct Keyboard { public ushort VirtualKey, Scan; public uint Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Sequential)] public struct Mouse { public int X,Y; public uint Data,Flags,Time; public UIntPtr Extra; }
    [DllImport("user32.dll", SetLastError=true)] static extern uint SendInput(uint count, Input[] input, int size);
    [DllImport("user32.dll")] static extern uint MapVirtualKeyW(uint key, uint mode);
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr window, uint flags);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    public static void FocusByClick(IntPtr form, int x, int y) {
        var point = new Point {X=x, Y=y};
        if (GetAncestor(WindowFromPoint(point), 2) != form) throw new InvalidOperationException("Probe occluded; refusing click.");
        if (!SetCursorPos(x,y)) throw new InvalidOperationException("Cannot position probe pointer.");
        var down = new Input(); down.Value.Mouse.Flags = 2;
        var up = down; up.Value.Mouse.Flags = 4;
        if (SendInput(2, new Input[] {down,up}, Marshal.SizeOf(typeof(Input))) != 2) throw new InvalidOperationException("Mouse injection failed.");
    }
    static Input KeyInput(ushort key, bool up) {
        var input = new Input(); input.Type = 1;
        input.Value.Keyboard.Scan = (ushort)MapVirtualKeyW(key,0);
        input.Value.Keyboard.Flags = 8u | (up ? 2u : 0u) | ((key >= 0x21 && key <= 0x28) ? 1u : 0u);
        return input;
    }
    public static void Key(IntPtr form, IntPtr control, ushort key, ushort modifier) {
        if (GetForegroundWindow() != form || GetFocus() != control) throw new InvalidOperationException("Probe lost focus; no input injected.");
        var keys = new List<Input>();
        if (modifier != 0) keys.Add(KeyInput(modifier,false));
        keys.Add(KeyInput(key,false)); keys.Add(KeyInput(key,true));
        if (modifier != 0) keys.Add(KeyInput(modifier,true));
        if (SendInput((uint)keys.Count, keys.ToArray(), Marshal.SizeOf(typeof(Input))) != keys.Count) throw new InvalidOperationException("Key injection failed.");
    }
    [DllImport("C:\\Windows\\System32\\icu.dll", CallingConvention=CallingConvention.Cdecl, CharSet=CharSet.Ansi, ExactSpelling=true)] static extern IntPtr ubrk_open(int kind, string locale, IntPtr text, int length, ref int status);
    [DllImport("C:\\Windows\\System32\\icu.dll", CallingConvention=CallingConvention.Cdecl, ExactSpelling=true)] static extern int ubrk_first(IntPtr iterator);
    [DllImport("C:\\Windows\\System32\\icu.dll", CallingConvention=CallingConvention.Cdecl, ExactSpelling=true)] static extern int ubrk_next(IntPtr iterator);
    [DllImport("C:\\Windows\\System32\\icu.dll", CallingConvention=CallingConvention.Cdecl, ExactSpelling=true)] static extern void ubrk_close(IntPtr iterator);
    public static int[] Boundaries(string text) {
        GCHandle pin = GCHandle.Alloc(text, GCHandleType.Pinned);
        IntPtr iterator = IntPtr.Zero;
        try {
            int status = 0; iterator = ubrk_open(0, "root", pin.AddrOfPinnedObject(), text.Length, ref status);
            if (status > 0 || iterator == IntPtr.Zero) throw new InvalidOperationException("ICU open failed: " + status);
            var positions = new List<int>();
            for (int position = ubrk_first(iterator); position != -1; position = ubrk_next(iterator)) positions.Add(position);
            return positions.ToArray();
        } finally { if (iterator != IntPtr.Zero) ubrk_close(iterator); pin.Free(); }
    }
}
'@
$fixture = Get-Content -LiteralPath (Join-Path $PSScriptRoot '..\fixtures\text.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$previousDpi = [NativeTextProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
if ($previousDpi -eq [IntPtr]::Zero) { throw 'Cannot enable per-monitor-v2 context.' }
$module = [NativeTextProbe]::LoadLibraryExW('msftedit.dll',[IntPtr]::Zero,0x800)
if ($module -eq [IntPtr]::Zero) { throw 'Cannot load system msftedit.dll.' }
[Windows.Forms.Application]::EnableVisualStyles()
$form = New-Object Windows.Forms.Form
$form.Text = 'Native RichEdit capability probe - not candidate correctness'
$form.StartPosition = 'CenterScreen'
$form.AutoScaleMode = 'None'
$form.ClientSize = New-Object Drawing.Size(900,540)
$form.TopMost = $true
$inputWindow = [NativeTextProbe]::CreateWindowExW(0x200,'RICHEDIT50W','',0x50B011C4,12,12,876,120,$form.Handle,[IntPtr]::Zero,[IntPtr]::Zero,[IntPtr]::Zero)
$response = [NativeTextProbe]::CreateWindowExW(0x200,'RICHEDIT50W','',0x50B019C4,12,144,876,384,$form.Handle,[IntPtr]::Zero,[IntPtr]::Zero,[IntPtr]::Zero)
if ($inputWindow -eq [IntPtr]::Zero -or $response -eq [IntPtr]::Zero) { throw 'Native RichEdit creation failed.' }
$font = [NativeTextProbe]::CreateFontW(-32,0,0,0,400,0,0,0,1,0,0,5,0,'Segoe UI')
foreach ($window in @($inputWindow,$response)) {
    if ([NativeTextProbe]::SendMessageW($window,0x459,[IntPtr](0x29),[IntPtr]::Zero) -ne [IntPtr]::Zero) { throw 'Cannot set RichEdit plain-text mode.' }
    [void][NativeTextProbe]::SendMessageW($window,0x30,$font,[IntPtr](1))
    [void][NativeTextProbe]::SendMessageW($window,0x4CA,[IntPtr](1),[IntPtr](1))
}
[void][NativeTextProbe]::SendMessageW($inputWindow,0x452,[IntPtr](32),[IntPtr]::Zero)
[void][NativeTextProbe]::SendMessageW($response,0x452,[IntPtr]::Zero,[IntPtr]::Zero)
$responseText = @($fixture.F1,$fixture.F2,$fixture.F3) + @($fixture.F4)
[void][NativeTextProbe]::SetWindowTextW($response,($responseText -join "`r`n"))
$script:record = [ordered]@{
    schema = 'mascot-native-text-capability-1'
    utc_started = [DateTime]::UtcNow.ToString('o')
    scope = 'Native stack investigation before candidate implementation; does not change frozen fixtures or grant candidate passes'
    native_class = 'RICHEDIT50W'
    font = 'Segoe UI, physical height 32'
    input_dpi = [NativeTextProbe]::GetDpiForWindow($inputWindow)
    text_mode = 'TM_PLAINTEXT | TM_MULTILEVELUNDO | TM_MULTICODEPAGE'
    typography_options = 'TO_ADVANCEDTYPOGRAPHY'
    msftedit_version = [Diagnostics.FileVersionInfo]::GetVersionInfo('C:\Windows\System32\msftedit.dll').FileVersion
    icu_version = [Diagnostics.FileVersionInfo]::GetVersionInfo('C:\Windows\System32\icu.dll').FileVersion
    icu_F1_boundaries = @([NativeTextProbe]::Boundaries($fixture.F1))
    icu_F2_boundaries = @([NativeTextProbe]::Boundaries($fixture.F2))
    native = [ordered]@{}
    captures = @()
    error = $null
}
$previousCursor = [Windows.Forms.Cursor]::Position
$previousClipboard = [Windows.Forms.Clipboard]::GetDataObject()
function Capture-TextProbe([string]$Name) {
    if ([NativeTextProbe]::GetForegroundWindow() -ne $form.Handle) { throw 'Probe not foreground at capture.' }
    $bounds = $form.Bounds
    $image = New-Object Drawing.Bitmap($bounds.Width,$bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($image)
    try {
        $graphics.CopyFromScreen($bounds.Location,[Drawing.Point]::Empty,$bounds.Size)
        $image.Save((Join-Path $OutputDirectory ($Name + '.png')),[Drawing.Imaging.ImageFormat]::Png)
        $script:record.captures += ($Name + '.png')
    } finally { $graphics.Dispose(); $image.Dispose() }
}
$captureTextProbe = ${function:Capture-TextProbe}
$script:actions = New-Object 'Collections.Generic.Queue[scriptblock]'
$script:actions.Enqueue({
    [void][NativeTextProbe]::SetForegroundWindow($form.Handle)
    $point = $form.PointToScreen((New-Object Drawing.Point(32,32)))
    [NativeTextProbe]::FocusByClick($form.Handle,$point.X,$point.Y)
    [void][NativeTextProbe]::SetFocus($inputWindow)
})
foreach ($name in @('F1','F2')) {
    $caseName = $name
    $caseText = $fixture.$name
    $script:actions.Enqueue({
        [void][NativeTextProbe]::SetWindowTextW($inputWindow,$caseText)
        $script:record.native[$caseName] = [ordered]@{initial=$caseText; positions=@(); copied=$null}
    }.GetNewClosure())
    $script:actions.Enqueue({ [NativeTextProbe]::Key($form.Handle,$inputWindow,0x23,0x11) })
    $script:actions.Enqueue({
        $script:record.native[$caseName].positions += [NativeTextProbe]::Selection($inputWindow)
        [NativeTextProbe]::Key($form.Handle,$inputWindow,0x25,0)
    }.GetNewClosure())
    $script:actions.Enqueue({
        $script:record.native[$caseName].positions += [NativeTextProbe]::Selection($inputWindow)
        [NativeTextProbe]::Key($form.Handle,$inputWindow,0x25,0)
    }.GetNewClosure())
    $script:actions.Enqueue({
        $script:record.native[$caseName].positions += [NativeTextProbe]::Selection($inputWindow)
        [NativeTextProbe]::Key($form.Handle,$inputWindow,0x27,0x10)
    }.GetNewClosure())
    $script:actions.Enqueue({
        $script:record.native[$caseName].positions += [NativeTextProbe]::Selection($inputWindow)
        [NativeTextProbe]::Key($form.Handle,$inputWindow,0x43,0x11)
    }.GetNewClosure())
    $script:actions.Enqueue({
        $script:record.native[$caseName].copied = [Windows.Forms.Clipboard]::GetText()
        & $captureTextProbe ($caseName + '-native-selection')
    }.GetNewClosure())
}
$script:actions.Enqueue({ $form.Close() })
$timer = New-Object Windows.Forms.Timer
$timer.Interval = 400
$timer.Add_Tick({
    try { if ($script:actions.Count -gt 0) { & ($script:actions.Dequeue()) } else { $form.Close() } }
    catch { $script:record.error = $_.Exception.Message; $form.Close() }
})
$form.Add_Shown({ $timer.Start() })
try { [Windows.Forms.Application]::Run($form) }
finally {
    $timer.Stop(); $timer.Dispose()
    $form.Dispose()
    [void][NativeTextProbe]::DeleteObject($font)
    [void][NativeTextProbe]::FreeLibrary($module)
    [void][NativeTextProbe]::SetThreadDpiAwarenessContext($previousDpi)
    [Windows.Forms.Cursor]::Position = $previousCursor
    if ($null -ne $previousClipboard) { [Windows.Forms.Clipboard]::SetDataObject($previousClipboard,$true) }
    $script:record.utc_finished = [DateTime]::UtcNow.ToString('o')
    $script:record | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'observations.json') -Encoding UTF8
}
if ($script:record.error) { throw $script:record.error }
Write-Output (Join-Path $OutputDirectory 'observations.json')
