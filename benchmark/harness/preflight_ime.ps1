param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
if ([Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') { throw 'Run with Windows PowerShell -STA.' }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new evidence directory; existing evidence is never overwritten.' }
[void][IO.Directory]::CreateDirectory($OutputDirectory)
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

public sealed class ImeProbeBox : TextBox {
    public readonly List<string> CompositionEvents = new List<string>();
    public bool Composing;
    public int Starts, Ends;
    public string Preedit = "";
    public string LastResult = "";
    [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr hwnd);
    [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr hwnd, IntPtr context);
    [DllImport("imm32.dll")] public static extern bool ImmSetOpenStatus(IntPtr context, bool open);
    [DllImport("imm32.dll")] public static extern bool ImmGetOpenStatus(IntPtr context);
    [DllImport("imm32.dll")] public static extern bool ImmSetConversionStatus(IntPtr context, int conversion, int sentence);
    [DllImport("imm32.dll")] static extern int ImmGetCompositionStringW(IntPtr context, int index, byte[] data, int bytes);
    public string CompositionString(int index) {
        IntPtr context = ImmGetContext(Handle);
        try {
            if (context == IntPtr.Zero) return "";
            int size = ImmGetCompositionStringW(context, index, null, 0);
            if (size <= 0 || size > 8192) return "";
            byte[] data = new byte[size];
            int read = ImmGetCompositionStringW(context, index, data, size);
            return read > 0 ? Encoding.Unicode.GetString(data, 0, read) : "";
        } finally { if (context != IntPtr.Zero) ImmReleaseContext(Handle, context); }
    }
    protected override void WndProc(ref Message message) {
        if (message.Msg == 0x010D) { Composing = true; Starts++; CompositionEvents.Add("WM_IME_STARTCOMPOSITION"); }
        if (message.Msg == 0x010F) {
            long flags = message.LParam.ToInt64();
            if ((flags & 8) != 0) { Preedit = CompositionString(8); CompositionEvents.Add("PREEDIT:" + Preedit); }
            if ((flags & 0x800) != 0) { LastResult = CompositionString(0x800); CompositionEvents.Add("RESULT:" + LastResult); }
        }
        if (message.Msg == 0x010E) { Composing = false; Ends++; CompositionEvents.Add("WM_IME_ENDCOMPOSITION"); }
        base.WndProc(ref message);
    }
}

[ComImport, Guid("71c6e74c-0f28-11d8-a82a-00065b84435c"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface ImeProfileManager {
    [PreserveSig] int ActivateProfile(uint type, ushort language, ref Guid service, ref Guid profile, IntPtr layout, uint flags);
}

public static class ImeProbeInput {
    public static int ActivateJapanese() {
        object instance = Activator.CreateInstance(Type.GetTypeFromCLSID(new Guid("33c53a50-f456-4884-b049-85fd643ecfed")));
        try {
            Guid service = new Guid("03B5835F-F03C-411B-9CE2-AA23E1171E36");
            Guid profile = new Guid("A76C93D9-5523-4E90-AAFA-4DB112F9AC76");
            return ((ImeProfileManager)instance).ActivateProfile(1, 0x0411, ref service, ref profile, IntPtr.Zero, 0x10000000);
        } finally { Marshal.ReleaseComObject(instance); }
    }
    [StructLayout(LayoutKind.Sequential)] public struct Input { public uint Type; public InputUnion Value; }
    [StructLayout(LayoutKind.Explicit)] public struct InputUnion {
        [FieldOffset(0)] public Keyboard Keyboard;
        [FieldOffset(0)] public Mouse Mouse;
    }
    [StructLayout(LayoutKind.Sequential)] public struct Keyboard { public ushort VirtualKey, Scan; public uint Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Sequential)] public struct Mouse { public int X,Y; public uint Data,Flags,Time; public UIntPtr Extra; }
    [DllImport("user32.dll", SetLastError=true)] static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] static extern uint MapVirtualKeyW(uint key, uint mode);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint process);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int max);
    public static string ForegroundDescription() {
        IntPtr window = GetForegroundWindow();
        uint process; GetWindowThreadProcessId(window, out process);
        var name = new StringBuilder(256); GetClassNameW(window, name, name.Capacity);
        return "hwnd=" + window.ToInt64().ToString("X") + "; pid=" + process + "; class=" + name;
    }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr window, uint flags);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    public static void FocusByClick(IntPtr window, int x, int y) {
        Point point = new Point { X=x, Y=y };
        if (GetAncestor(WindowFromPoint(point), 2) != window)
            throw new InvalidOperationException("Probe click is occluded; refusing input.");
        if (!SetCursorPos(x,y)) throw new InvalidOperationException("Cannot position probe pointer.");
        Input down = new Input(); down.Value.Mouse.Flags = 2;
        Input up = down; up.Value.Mouse.Flags = 4;
        if (SendInput(2, new Input[] {down,up}, Marshal.SizeOf(typeof(Input))) != 2)
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
    public static void Key(IntPtr window, ushort key) {
        if (GetForegroundWindow() != window) SetForegroundWindow(window);
        if (GetForegroundWindow() != window) throw new InvalidOperationException("Probe lost foreground; no input injected. " + ForegroundDescription() + "; expected=" + window.ToInt64().ToString("X"));
        Input down = new Input(); down.Type = 1; down.Value.Keyboard.Scan = (ushort)MapVirtualKeyW(key,0); down.Value.Keyboard.Flags = 8;
        Input up = down; up.Value.Keyboard.Flags = 10;
        if (SendInput(2, new Input[] {down, up}, Marshal.SizeOf(typeof(Input))) != 2)
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@

$previousDpiContext = [ImeProbeInput]::SetThreadDpiAwarenessContext([IntPtr](-4))
if ($previousDpiContext -eq [IntPtr]::Zero) { throw 'Cannot enable per-monitor-v2 awareness for reliable capture coordinates.' }
[Windows.Forms.Application]::EnableVisualStyles()
$form = New-Object Windows.Forms.Form
$form.Text = 'Mascot benchmark: Japanese IME preflight only'
$form.StartPosition = 'CenterScreen'
$form.ClientSize = New-Object Drawing.Size(720,300)
$form.TopMost = $true
$label = New-Object Windows.Forms.Label
$label.Text = 'Microsoft Japanese IME: physical romaji input; composition / commit / cancel'
$label.Dock = 'Top'
$label.Height = 42
$box = New-Object ImeProbeBox
$box.Dock = 'Fill'
$box.Font = New-Object Drawing.Font('Yu Gothic UI',24)
$box.ImeMode = 'On'
$form.Controls.Add($box)
$form.Controls.Add($label)
$script:record = [ordered]@{
    schema = 'mascot-ime-preflight-1'
    utc_started = [DateTime]::UtcNow.ToString('o')
    method = 'Microsoft Japanese IME; actual SendInput scan codes, no Japanese Unicode injection'
    probe_control = 'Stock Win32 single-line EDIT via Windows Forms; environment preflight, not candidate correctness'
    romaji = 'nihonn'
    expected_commit = ([string][char]0x306B + [char]0x307B + [char]0x3093)
    prior_text_for_cancel = 'baseline'
    installed_input_languages = @()
    events = @()
    captures = @()
    error = $null
    visually_reviewed = $false
}
$script:previousLanguage = [Windows.Forms.InputLanguage]::CurrentInputLanguage
$previousCursor = [Windows.Forms.Cursor]::Position
$script:japanese = $null
foreach ($language in [Windows.Forms.InputLanguage]::InstalledInputLanguages) {
    $script:record.installed_input_languages += [ordered]@{ culture=$language.Culture.Name; layout=$language.LayoutName; handle=$language.Handle.ToInt64().ToString('X') }
    if ($language.Culture.Name -eq 'ja-JP') { $script:japanese = $language }
}
$script:record.japanese_layout_available = $null -ne $script:japanese
function Save-ProbeCapture([string]$Name) {
    if ([ImeProbeInput]::GetForegroundWindow() -ne $form.Handle) { throw 'Probe not foreground at capture.' }
    $bounds = $form.Bounds
    $image = New-Object Drawing.Bitmap($bounds.Width,$bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($image)
    try {
        $graphics.CopyFromScreen($bounds.Location,[Drawing.Point]::Empty,$bounds.Size)
        $path = Join-Path $OutputDirectory ($Name + '.png')
        $image.Save($path,[Drawing.Imaging.ImageFormat]::Png)
        $script:record.captures += ($Name + '.png')
    } finally { $graphics.Dispose(); $image.Dispose() }
}
$script:actions = New-Object 'Collections.Generic.Queue[scriptblock]'
$script:actions.Enqueue({
    if ($null -eq $script:japanese) { throw 'Japanese input language not installed; do not freeze fixture.' }
    [Windows.Forms.InputLanguage]::CurrentInputLanguage = $script:japanese
    [void][ImeProbeInput]::SetForegroundWindow($form.Handle)
    [void]$box.Focus()
    $script:record.tsf_profile_activation_hresult = [ImeProbeInput]::ActivateJapanese()
    if ($script:record.tsf_profile_activation_hresult -lt 0) { throw 'Microsoft Japanese TSF profile activation failed.' }
})
$script:actions.Enqueue({
    $point = $box.PointToScreen((New-Object Drawing.Point(20,20)))
    [ImeProbeInput]::FocusByClick($form.Handle,$point.X,$point.Y)
})
$script:actions.Enqueue({
    $context = [ImeProbeBox]::ImmGetContext($box.Handle)
    if ($context -eq [IntPtr]::Zero) { throw 'No IME input context on probe.' }
    try {
        $script:record.open_status_set = [ImeProbeBox]::ImmSetOpenStatus($context,$true)
        $script:record.conversion_status_set = [ImeProbeBox]::ImmSetConversionStatus($context,0x19,0)
        $script:record.open_status = [ImeProbeBox]::ImmGetOpenStatus($context)
    } finally { [void][ImeProbeBox]::ImmReleaseContext($box.Handle,$context) }
})
foreach ($key in @(0x4E,0x49,0x48,0x4F,0x4E,0x4E)) {
    $capturedKey = [uint16]$key
    $script:actions.Enqueue({ [ImeProbeInput]::Key($form.Handle,$capturedKey) }.GetNewClosure())
}
$script:actions.Enqueue({
    $script:record.preedit = $box.CompositionString(8)
    $script:record.composing_before_commit = $box.Composing
    $script:record.text_before_commit = $box.Text
    Save-ProbeCapture '01-preedit'
})
$script:actions.Enqueue({ [ImeProbeInput]::Key($form.Handle,0x0D) })
$script:actions.Enqueue({
    $script:record.committed_text = $box.Text
    $script:record.composing_after_commit = $box.Composing
    Save-ProbeCapture '02-committed'
    $box.Text = 'baseline'
    $box.SelectionStart = $box.TextLength
})
foreach ($key in @(0x4E,0x49,0x48,0x4F,0x4E,0x4E)) {
    $capturedKey = [uint16]$key
    $script:actions.Enqueue({ [ImeProbeInput]::Key($form.Handle,$capturedKey) }.GetNewClosure())
}
$script:actions.Enqueue({
    $script:record.cancel_preedit = $box.CompositionString(8)
    $script:record.composing_before_cancel = $box.Composing
    Save-ProbeCapture '03-before-cancel'
})
$script:actions.Enqueue({ [ImeProbeInput]::Key($form.Handle,0x1B) })
$script:actions.Enqueue({ [ImeProbeInput]::Key($form.Handle,0x1B) })
$script:actions.Enqueue({
    $script:record.text_after_cancel = $box.Text
    $script:record.composing_after_cancel = $box.Composing
    Save-ProbeCapture '04-cancelled'
    $form.Close()
})
$timer = New-Object Windows.Forms.Timer
$timer.Interval = 400
$timer.Add_Tick({
    try {
        if ($script:actions.Count -gt 0) { & ($script:actions.Dequeue()) }
        else { $form.Close() }
    } catch {
        $script:record.error = $_.Exception.Message
        $form.Close()
    }
})
$form.Add_Shown({ $timer.Start() })
try { [Windows.Forms.Application]::Run($form) }
finally {
    $timer.Stop()
    $timer.Dispose()
    $script:record.events = @($box.CompositionEvents)
    $script:record.composition_starts = $box.Starts
    $script:record.composition_ends = $box.Ends
    $script:record.utc_finished = [DateTime]::UtcNow.ToString('o')
    [Windows.Forms.InputLanguage]::CurrentInputLanguage = $script:previousLanguage
    [Windows.Forms.Cursor]::Position = $previousCursor
    $form.Dispose()
    [void][ImeProbeInput]::SetThreadDpiAwarenessContext($previousDpiContext)
    $script:record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'observations.json') -Encoding UTF8
}
if ($script:record.error) { throw $script:record.error }
Write-Output (Join-Path $OutputDirectory 'observations.json')
