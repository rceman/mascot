package main

// Direct Win32 bindings via system-DLL lazy loaders. No cgo. Callbacks are
// created once at process start (NewCallback trampolines are process-lifetime
// resources) and reused for the life of the program.

import (
	"unicode/utf16"
	"unsafe"

	"golang.org/x/sys/windows"
)

var (
	user32   = windows.NewLazySystemDLL("user32.dll")
	gdi32    = windows.NewLazySystemDLL("gdi32.dll")
	comctl32 = windows.NewLazySystemDLL("comctl32.dll")
	imm32    = windows.NewLazySystemDLL("imm32.dll")
	ole32    = windows.NewLazySystemDLL("ole32.dll")
	kernel32 = windows.NewLazySystemDLL("kernel32.dll")
)

var (
	procRegisterClassW                 = user32.NewProc("RegisterClassW")
	procCreateWindowExW                = user32.NewProc("CreateWindowExW")
	procDefWindowProcW                 = user32.NewProc("DefWindowProcW")
	procGetMessageW                    = user32.NewProc("GetMessageW")
	procTranslateMessage               = user32.NewProc("TranslateMessage")
	procDispatchMessageW               = user32.NewProc("DispatchMessageW")
	procPostMessageW                   = user32.NewProc("PostMessageW")
	procPostQuitMessage                = user32.NewProc("PostQuitMessage")
	procShowWindow                     = user32.NewProc("ShowWindow")
	procSetWindowPos                   = user32.NewProc("SetWindowPos")
	procGetWindowRect                  = user32.NewProc("GetWindowRect")
	procGetClientRect                  = user32.NewProc("GetClientRect")
	procAdjustWindowRectExForDpi       = user32.NewProc("AdjustWindowRectExForDpi")
	procGetDpiForWindow                = user32.NewProc("GetDpiForWindow")
	procGetDpiForSystem                = user32.NewProc("GetDpiForSystem")
	procMonitorFromWindow              = user32.NewProc("MonitorFromWindow")
	procMonitorFromPoint               = user32.NewProc("MonitorFromPoint")
	procGetMonitorInfoW                = user32.NewProc("GetMonitorInfoW")
	procGetCursorPos                   = user32.NewProc("GetCursorPos")
	procLoadCursorW                    = user32.NewProc("LoadCursorW")
	procRegisterHotKey                 = user32.NewProc("RegisterHotKey")
	procUnregisterHotKey               = user32.NewProc("UnregisterHotKey")
	procGetKeyState                    = user32.NewProc("GetKeyState")
	procCreatePopupMenu                = user32.NewProc("CreatePopupMenu")
	procAppendMenuW                    = user32.NewProc("AppendMenuW")
	procTrackPopupMenu                 = user32.NewProc("TrackPopupMenu")
	procDestroyMenu                    = user32.NewProc("DestroyMenu")
	procSetForegroundWindow            = user32.NewProc("SetForegroundWindow")
	procSetFocus                       = user32.NewProc("SetFocus")
	procIsWindowVisible                = user32.NewProc("IsWindowVisible")
	procGetDC                          = user32.NewProc("GetDC")
	procReleaseDC                      = user32.NewProc("ReleaseDC")
	procUpdateLayeredWindow            = user32.NewProc("UpdateLayeredWindow")
	procSetWindowTextW                 = user32.NewProc("SetWindowTextW")
	procSendMessageW                   = user32.NewProc("SendMessageW")
	procGetWindowLongPtrW              = user32.NewProc("GetWindowLongPtrW")
	procSetWindowLongPtrW              = user32.NewProc("SetWindowLongPtrW")
	procDestroyWindow                  = user32.NewProc("DestroyWindow")
	procSetThreadDpiAwarenessContext   = user32.NewProc("SetThreadDpiAwarenessContext")
	procSetProcessDpiAwarenessContext  = user32.NewProc("SetProcessDpiAwarenessContext")
	procGetThreadDpiAwarenessContext   = user32.NewProc("GetThreadDpiAwarenessContext")
	procAreDpiAwarenessContextsEqual   = user32.NewProc("AreDpiAwarenessContextsEqual")
	procGetSysColorBrush               = user32.NewProc("GetSysColorBrush")
	procCreateCompatibleDC             = gdi32.NewProc("CreateCompatibleDC")
	procCreateDIBSection               = gdi32.NewProc("CreateDIBSection")
	procSelectObject                   = gdi32.NewProc("SelectObject")
	procDeleteObject                   = gdi32.NewProc("DeleteObject")
	procDeleteDC                       = gdi32.NewProc("DeleteDC")
	procCreateFontW                    = gdi32.NewProc("CreateFontW")
	procSetWindowSubclass              = comctl32.NewProc("SetWindowSubclass")
	procRemoveWindowSubclass           = comctl32.NewProc("RemoveWindowSubclass")
	procDefSubclassProc                = comctl32.NewProc("DefSubclassProc")
	procImmGetContext                  = imm32.NewProc("ImmGetContext")
	procImmNotifyIME                   = imm32.NewProc("ImmNotifyIME")
	procImmReleaseContext              = imm32.NewProc("ImmReleaseContext")
	procOleInitialize                  = ole32.NewProc("OleInitialize")
	procOleUninitialize                = ole32.NewProc("OleUninitialize")
	procQueryPerformanceCounter        = kernel32.NewProc("QueryPerformanceCounter")
	procQueryPerformanceFrequency      = kernel32.NewProc("QueryPerformanceFrequency")
	procCancelSynchronousIo            = kernel32.NewProc("CancelSynchronousIo")
	procGetModuleHandleExW             = kernel32.NewProc("GetModuleHandleExW")
	procGetLastError                   = kernel32.NewProc("GetLastError")
	procLoadLibraryExW                 = kernel32.NewProc("LoadLibraryExW")
	procFreeLibrary                    = kernel32.NewProc("FreeLibrary")
)

const (
	wsPOPUP         = 0x80000000
	wsCAPTION       = 0x00C00000
	wsSYSMENU       = 0x00080000
	wsMINIMIZEBOX   = 0x00020000
	wsCLIPCHILDREN  = 0x02000000
	wsCHILD         = 0x40000000
	wsVISIBLE       = 0x10000000
	wsVSCROLL       = 0x00200000
	wsEXLAYERED     = 0x00080000
	wsEXTOPMOST     = 0x00000008
	wsEXTOOLWINDOW  = 0x00000080
	wsEXNOACTIVATE  = 0x08000000
	swHIDE          = 0
	swSHOWNORMAL    = 1
	swSHOWNOACTIVATE = 4

	wmCREATE               = 0x0001
	wmDESTROY              = 0x0002
	wmCLOSE                = 0x0010
	wmPAINT                = 0x000F
	wmSETFONT              = 0x0030
	wmNCCREATE             = 0x0081
	wmNCDESTROY            = 0x0082
	wmNCHITTEST            = 0x0084
	wmKEYDOWN              = 0x0100
	wmRBUTTONUP            = 0x0205
	wmNCRBUTTONUP          = 0x00A5
	wmCOMMAND              = 0x0111
	wmIMESTARTCOMPOSITION  = 0x010D
	wmIMEENDCOMPOSITION    = 0x010E
	wmIMECOMPOSITION       = 0x010F
	wmAPP                  = 0x8000
	wmAPPPROVIDER          = wmAPP + 1
	wmAPPCONTROL           = wmAPP + 2
	wmAPPSUBMIT            = wmAPP + 3
	wmHOTKEY               = 0x0312
	wmDPICHANGED           = 0x02E0

	htTRANSPARENT = -1
	htCAPTION     = 2

	vkRETURN  = 0x0D
	vkCONTROL = 0x11
	vkSPACE   = 0x20
	vkESCAPE  = 0x1B

	modNOREPEAT = 0x4000
	modCONTROL  = 0x0002
	modALT      = 0x0001

	hotkeyToggleID = 1
	hotkeyCancelID = 2

	monitorDEFAULTTONEAREST = 2
	gwlpUSERDATA            = -21

	biRGB         = 0
	dibRGBCOLORS  = 0
	acSRCOVER     = 0
	acSRCALPHA    = 1
	ulwALPHA      = 2

	bsPUSHBUTTON   = 0
	ssLEFT         = 0
	bnCLICKED      = 0
	mfSTRING       = 0
	tpmRETURNCMD   = 0x0100
	tpmRIGHTBUTTON = 0x0002
	tpmNONOTIFY    = 0x0080
	swpNOZORDER    = 0x0004
	swpNOACTIVATE  = 0x0010
	colorWINDOW    = 5
	idcARROW       = 32512

	fwNORMAL           = 400
	defaultCHARSET     = 1
	outDEFAULTPRECIS   = 0
	clipDEFAULTPRECIS  = 0
	cleartypeQUALITY   = 5
	defaultPITCH       = 0
	ffDONTCARE         = 0

	loadLIBRARYSEARCHSYSTEM32 = 0x00000800

	cpsCANCEL        = 4
	niCOMPOSITIONSTR = 0x15

	duplicateSAMEACCESS = 0x0002

	waitOBJECT0 = 0

	dpiAwarenessContextPerMonitorAwareV2 = ^uintptr(3) // (HANDLE)-4

	idMenuExit = 4001
)

// RichEdit messages/flags (mirrors rust/src/text.rs).
const (
	emGETTEXTEX           = 0x0400 + 94        // WM_USER + 94
	emGETTEXTLENGTHEX     = 0x0400 + 95        // WM_USER + 95
	emSETTEXTEX           = 0x0400 + 97
	emSETTEXTMODE         = 0x0400 + 89
	emSETLIMITTEXT        = 0x00C5
	emSETUNDOLIMIT        = 0x0400 + 82
	emSETTYPOGRAPHYOPTIONS = 0x0400 + 202
	emSETSEL              = 0x00B1
	emREPLACESEL          = 0x00C2
	emSCROLLCARET         = 0x00B7
	emEXGETSEL            = 0x0400 + 52
	emEXSETSEL            = 0x0400 + 55

	tmPLAINTEXT      = 1
	tmMULTILEVELUNDO = 8
	tmMULTICODEPAGE  = 32
	toADVANCEDTYPOGRAPHY = 1

	esMULTILINE  = 0x0004
	esWANTRETURN = 0x1000
	esREADONLY   = 0x0800
	esNOHIDESEL  = 0x0100
)

type rect struct {
	Left, Top, Right, Bottom int32
}

type point struct {
	X, Y int32
}

type size struct {
	CX, CY int32
}

type msg struct {
	Hwnd    uintptr
	Message uint32
	WParam  uintptr
	LParam  uintptr
	Time    uint32
	Pt      point
}

type wndclassw struct {
	Style         uint32
	LpfnWndProc   uintptr
	CbClsExtra    int32
	CbWndExtra    int32
	HInstance     uintptr
	HIcon         uintptr
	HCursor       uintptr
	HbrBackground uintptr
	LpszMenuName  *uint16
	LpszClassName *uint16
}

type bitmapinfoheader struct {
	BiSize          uint32
	BiWidth         int32
	BiHeight        int32
	BiPlanes        uint16
	BiBitCount      uint16
	BiCompression   uint32
	BiSizeImage     uint32
	BiXPelsPerMeter int32
	BiYPelsPerMeter int32
	BiClrUsed       uint32
	BiClrImportant  uint32
}

type rgbquad struct {
	RgbBlue, RgbGreen, RgbRed, RgbReserved byte
}

type bitmapinfo struct {
	BmiHeader bitmapinfoheader
	BmiColors [1]rgbquad
}

type blendfunction struct {
	BlendOp, BlendFlags, SourceConstantAlpha, AlphaFormat byte
}

type monitorinfo struct {
	CbSize    uint32
	RcMonitor rect
	RcWork    rect
	DwFlags   uint32
}

type createstructw struct {
	LpCreateParams uintptr
	HInstance      uintptr
	HMenu          uintptr
	HwndParent     uintptr
	CY, CX, Y, X   int32
	Style          int32
	LpszName       uintptr
	LpszClass      uintptr
	DwExStyle      uint32
}

type charrange struct {
	Min, Max int32
}

type settextex struct {
	Flags    uint32
	Codepage uint32
}

type getTextEx struct {
	Cb          uint32
	Flags       uint32
	Codepage    uint32
	DefaultChar *byte
	UsedDefault *int32
}

type getTextLengthEx struct {
	Flags    uint32
	Codepage uint32
}

// wide converts s to a NUL-terminated UTF-16 slice. A NUL-containing string
// falls back to truncation-safe encoding (callers that must reject NUL do so
// earlier, matching the reference implementation).
func wide(s string) []uint16 {
	u, err := windows.UTF16FromString(s)
	if err != nil {
		out := make([]uint16, 0, len(s)+1)
		for _, r := range s {
			if r == 0 {
				continue
			}
			out = append(out, utf16.Encode([]rune{r})...)
		}
		out = append(out, 0)
		return out
	}
	return u
}

func widep(s string) *uint16 { return &wide(s)[0] }

func qpc() int64 {
	var v int64
	r, _, _ := procQueryPerformanceCounter.Call(uintptr(unsafe.Pointer(&v)))
	if r == 0 {
		panic("QueryPerformanceCounter failed")
	}
	return v
}

func qpcFrequency() int64 {
	var v int64
	r, _, _ := procQueryPerformanceFrequency.Call(uintptr(unsafe.Pointer(&v)))
	if r == 0 || v <= 0 {
		panic("QueryPerformanceFrequency failed")
	}
	return v
}

func moduleInstance() uintptr {
	var h uintptr
	procGetModuleHandleExW.Call(0, 0, uintptr(unsafe.Pointer(&h)))
	return h
}

func dip(value int32, dpi uint32) int32 {
	return (value*int32(dpi) + 48) / 96
}

// neg converts a negative 32-bit constant (e.g. GWLP_USERDATA, HTTRANSPARENT)
// to the uintptr bit pattern expected by Win32 without tripping constant
// overflow checks.
func neg(v int32) uintptr {
	return uintptr(uint64(int64(v)))
}

func fontHeight(dpi uint32) int32 { return -dip(16, dpi) }

func min64(a, b uint64) uint64 {
	if a < b {
		return a
	}
	return b
}

func sendMessage(hwnd uintptr, msg uint32, wparam, lparam uintptr) uintptr {
	r, _, _ := procSendMessageW.Call(hwnd, uintptr(msg), wparam, lparam)
	return r
}

func postMessage(hwnd uintptr, msg uint32, wparam, lparam uintptr) bool {
	r, _, _ := procPostMessageW.Call(hwnd, uintptr(msg), wparam, lparam)
	return r != 0
}
