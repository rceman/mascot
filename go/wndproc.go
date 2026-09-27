package main

// Window procedures and window/class creation. Callback trampolines are
// created exactly once (process lifetime) via windows.NewCallback.

import (
	"fmt"
	"os"
	"unsafe"

	"golang.org/x/sys/windows"
)

// Callback trampolines are process-lifetime resources: created exactly once in
// registerClasses before any window or subclass exists.
var (
	mascotWndProcPtr   uintptr
	composerWndProcPtr uintptr
	subclassProcPtr    uintptr
)

func defWindowProc(hwnd uintptr, message uint32, wparam, lparam uintptr) uintptr {
	r, _, _ := procDefWindowProcW.Call(hwnd, uintptr(message), wparam, lparam)
	return r
}

func uiFromWindow(hwnd uintptr) *UI {
	r, _, _ := procGetWindowLongPtrW.Call(hwnd, neg(gwlpUSERDATA))
	if r == 0 {
		return nil
	}
	return (*UI)(unsafe.Pointer(r))
}

func mascotWndProc(hwnd uintptr, message uint32, wparam, lparam uintptr) uintptr {
	if message == wmNCCREATE {
		cs := (*createstructw)(unsafe.Pointer(lparam))
		procSetWindowLongPtrW.Call(hwnd, neg(gwlpUSERDATA), cs.LpCreateParams)
		if cs.LpCreateParams != 0 {
			(*UI)(unsafe.Pointer(cs.LpCreateParams)).mascotHwnd = hwnd
		}
		return defWindowProc(hwnd, message, wparam, lparam)
	}
	ui := uiFromWindow(hwnd)
	if ui == nil {
		return defWindowProc(hwnd, message, wparam, lparam)
	}
	switch message {
	case wmAPPPROVIDER:
		ui.dispatchProviderEvents()
		return 0
	case wmAPPCONTROL:
		ui.dispatchControl()
		return 0
	case wmAPPSUBMIT:
		if err := ui.submit(); err != nil {
			fmt.Fprintln(os.Stderr, "submit rejected:", err)
		}
		return 0
	case wmHOTKEY:
		if int32(wparam) == hotkeyToggleID {
			visible := false
			if ui.composer != nil {
				r, _, _ := procIsWindowVisible.Call(ui.composer.hwnd)
				visible = r != 0
			}
			if visible {
				ui.hideComposer()
			} else if err := ui.showComposer(); err != nil {
				fmt.Fprintln(os.Stderr, "show failed:", err)
			}
		} else if int32(wparam) == hotkeyCancelID {
			_ = ui.cancelRequest()
		}
		return 0
	case wmNCHITTEST:
		x := int32(int16(lparam & 0xffff))
		y := int32(int16((lparam >> 16) & 0xffff))
		var r rect
		procGetWindowRect.Call(hwnd, uintptr(unsafe.Pointer(&r)))
		width := r.Right - r.Left
		height := r.Bottom - r.Top
		if width <= 0 || height <= 0 {
			return neg(htTRANSPARENT)
		}
		px := x - r.Left
		py := y - r.Top
		if px < 0 || py < 0 || px >= width || py >= height {
			return neg(htTRANSPARENT)
		}
		sx := min(int(int(px)*ui.mascotSrcW/int(width)), ui.mascotSrcW-1)
		sy := min(int(int(py)*ui.mascotSrcH/int(height)), ui.mascotSrcH-1)
		alpha := ui.mascotSource[(sy*ui.mascotSrcW+sx)*4+3]
		if alpha > 0 {
			return uintptr(htCAPTION)
		}
		return neg(htTRANSPARENT)
	case wmDPICHANGED:
		suggested := (*rect)(unsafe.Pointer(lparam))
		newDPI := uint32(wparam & 0xffff)
		sz := dip(int32(ui.cfg.manifest.Asset.LogicalWidthDip), newDPI)
		procSetWindowPos.Call(
			hwnd, 0,
			uintptr(suggested.Left), uintptr(suggested.Top),
			uintptr(sz), uintptr(sz),
			swpNOZORDER|swpNOACTIVATE,
		)
		if err := ui.presentMascot(newDPI); err != nil {
			ui.m.runInvalid = "mascot presentation failed: " + err.Error()
			ui.m.hasRunInvalid = true
		}
		return 0
	case wmRBUTTONUP, wmNCRBUTTONUP:
		menu, _, _ := procCreatePopupMenu.Call()
		if menu != 0 {
			procAppendMenuW.Call(menu, mfSTRING, idMenuExit, uintptr(unsafe.Pointer(widep("Exit"))))
			var pt point
			procGetCursorPos.Call(uintptr(unsafe.Pointer(&pt)))
			procSetForegroundWindow.Call(hwnd)
			choice, _, _ := procTrackPopupMenu.Call(
				menu, tpmRETURNCMD|tpmRIGHTBUTTON|tpmNONOTIFY,
				uintptr(pt.X), uintptr(pt.Y), 0, hwnd, 0)
			procDestroyMenu.Call(menu)
			if choice == idMenuExit {
				ui.requestShutdown(nil)
			}
		}
		return 0
	case wmCLOSE:
		ui.requestShutdown(nil)
		return 0
	case wmDESTROY:
		procPostQuitMessage.Call(0)
		return 0
	default:
		return defWindowProc(hwnd, message, wparam, lparam)
	}
}

func composerWndProc(hwnd uintptr, message uint32, wparam, lparam uintptr) uintptr {
	if message == wmNCCREATE {
		cs := (*createstructw)(unsafe.Pointer(lparam))
		procSetWindowLongPtrW.Call(hwnd, neg(gwlpUSERDATA), cs.LpCreateParams)
		return defWindowProc(hwnd, message, wparam, lparam)
	}
	ui := uiFromWindow(hwnd)
	if ui == nil {
		return defWindowProc(hwnd, message, wparam, lparam)
	}
	switch message {
	case wmCREATE:
		c, err := createChildren(ui, hwnd)
		if err != nil {
			return ^uintptr(0) // -1: fail creation
		}
		ui.composer = c
		return 0
	case wmCOMMAND:
		id := int(wparam & 0xffff)
		notify := uint32(wparam >> 16)
		if notify == bnCLICKED {
			if id == idSend {
				if err := ui.submit(); err != nil {
					fmt.Fprintln(os.Stderr, "submit rejected:", err)
				}
			} else if id == idCancel {
				_ = ui.cancelRequest()
			}
		}
		return 0
	case wmPAINT:
		ui.paints++
		return defWindowProc(hwnd, message, wparam, lparam)
	case wmDPICHANGED:
		suggested := (*rect)(unsafe.Pointer(lparam))
		newDPI := uint32(wparam & 0xffff)
		frame := rect{
			Right:  dip(int32(ui.cfg.manifest.UI.ComposerClientWidthDip), newDPI),
			Bottom: dip(int32(ui.cfg.manifest.UI.ComposerClientHeightDip), newDPI),
		}
		style := uint32(wsCAPTION | wsSYSMENU | wsMINIMIZEBOX | wsCLIPCHILDREN)
		procAdjustWindowRectExForDpi.Call(
			uintptr(unsafe.Pointer(&frame)), uintptr(style), 0, 0, uintptr(newDPI))
		procSetWindowPos.Call(
			hwnd, 0,
			uintptr(suggested.Left), uintptr(suggested.Top),
			uintptr(frame.Right-frame.Left), uintptr(frame.Bottom-frame.Top),
			swpNOZORDER|swpNOACTIVATE,
		)
		relayout(ui)
		return 0
	case wmCLOSE:
		ui.hideComposer()
		return 0
	default:
		return defWindowProc(hwnd, message, wparam, lparam)
	}
}

func createChildren(ui *UI, hwnd uintptr) (*composer, error) {
	module, err := loadRichedit()
	if err != nil {
		return nil, err
	}
	dpi := getDpiForWindow(hwnd)
	font, _, _ := procCreateFontW.Call(
		uintptr(fontHeight(dpi)), 0, 0, 0, fwNORMAL, 0, 0, 0,
		defaultCHARSET, outDEFAULTPRECIS, clipDEFAULTPRECIS, cleartypeQUALITY,
		defaultPITCH|ffDONTCARE,
		uintptr(unsafe.Pointer(widep("Segoe UI"))),
	)
	if font == 0 {
		procFreeLibrary.Call(module)
		procOleUninitialize.Call()
		return nil, errString("font creation failed")
	}
	instance := moduleInstance()
	uiConf := &ui.cfg.manifest.UI
	scale := func(v int32) int32 { return dip(v, dpi) }
	var created []uintptr
	cleanup := func() {
		for _, child := range created {
			procDestroyWindow.Call(child)
		}
		procDeleteObject.Call(font)
		procFreeLibrary.Call(module)
		procOleUninitialize.Call()
	}
	input, err := createEdit(hwnd, instance,
		scale(12), scale(12), scale(616), scale(int32(uiConf.InputHeightDip)),
		false, uintptr(uiConf.InputLimitUTF16Units), 32, font, idSend+10)
	if err != nil {
		cleanup()
		return nil, err
	}
	created = append(created, input)
	if !subclassControl(input, ui, true) {
		cleanup()
		return nil, errString("input subclass failed")
	}
	response, err := createEdit(hwnd, instance,
		scale(12), scale(152), scale(616), scale(int32(uiConf.ResponseHeightDip)),
		true, uintptr(uiConf.ResponseLimitUTF8Bytes)+8192, 0, font, idSend+11)
	if err != nil {
		cleanup()
		return nil, err
	}
	created = append(created, response)
	if !subclassControl(response, ui, false) {
		cleanup()
		return nil, errString("response subclass failed")
	}
	status, _, _ := procCreateWindowExW.Call(
		0,
		uintptr(unsafe.Pointer(widep("STATIC"))),
		uintptr(unsafe.Pointer(widep("idle"))),
		wsCHILD|wsVISIBLE|ssLEFT,
		uintptr(scale(12)), uintptr(scale(436)), uintptr(scale(416)), uintptr(scale(28)),
		hwnd, idStatus, instance, 0,
	)
	if status == 0 {
		cleanup()
		return nil, errString("status creation failed")
	}
	created = append(created, status)
	if !subclassControl(status, ui, false) {
		cleanup()
		return nil, errString("status subclass failed")
	}
	send, _, _ := procCreateWindowExW.Call(
		0,
		uintptr(unsafe.Pointer(widep("BUTTON"))),
		uintptr(unsafe.Pointer(widep("Send"))),
		wsCHILD|wsVISIBLE|bsPUSHBUTTON,
		uintptr(scale(440)), uintptr(scale(436)), uintptr(scale(80)), uintptr(scale(28)),
		hwnd, idSend, instance, 0,
	)
	if send == 0 {
		cleanup()
		return nil, errString("send button creation failed")
	}
	created = append(created, send)
	if !subclassControl(send, ui, false) {
		cleanup()
		return nil, errString("send subclass failed")
	}
	cancelBtn, _, _ := procCreateWindowExW.Call(
		0,
		uintptr(unsafe.Pointer(widep("BUTTON"))),
		uintptr(unsafe.Pointer(widep("Cancel"))),
		wsCHILD|wsVISIBLE|bsPUSHBUTTON,
		uintptr(scale(528)), uintptr(scale(436)), uintptr(scale(100)), uintptr(scale(28)),
		hwnd, idCancel, instance, 0,
	)
	if cancelBtn == 0 {
		cleanup()
		return nil, errString("cancel button creation failed")
	}
	created = append(created, cancelBtn)
	if !subclassControl(cancelBtn, ui, false) {
		cleanup()
		return nil, errString("cancel subclass failed")
	}
	sendMessage(status, wmSETFONT, font, 1)
	sendMessage(send, wmSETFONT, font, 1)
	sendMessage(cancelBtn, wmSETFONT, font, 1)
	setText(response, ui.cfg.historyPrefix)
	return &composer{
		hwnd: hwnd, input: input, response: response, status: status,
		send: send, cancel: cancelBtn, font: font, module: module, dpi: dpi,
	}, nil
}

func relayout(ui *UI) {
	hwnd, input, response, status, send, cancel, ok := ui.composerHandles()
	if !ok {
		return
	}
	dpi := getDpiForWindow(hwnd)
	font, _, _ := procCreateFontW.Call(
		uintptr(fontHeight(dpi)), 0, 0, 0, fwNORMAL, 0, 0, 0,
		defaultCHARSET, outDEFAULTPRECIS, clipDEFAULTPRECIS, cleartypeQUALITY,
		defaultPITCH|ffDONTCARE,
		uintptr(unsafe.Pointer(widep("Segoe UI"))),
	)
	if font != 0 {
		sendMessage(input, wmSETFONT, font, 1)
		sendMessage(response, wmSETFONT, font, 1)
		sendMessage(status, wmSETFONT, font, 1)
		sendMessage(send, wmSETFONT, font, 1)
		sendMessage(cancel, wmSETFONT, font, 1)
		if ui.composer != nil {
			procDeleteObject.Call(ui.composer.font)
			ui.composer.font = font
			ui.composer.dpi = dpi
		}
	}
	scale := func(v int32) int32 { return dip(v, dpi) }
	uiConf := &ui.cfg.manifest.UI
	flags := uintptr(swpNOZORDER | swpNOACTIVATE)
	procSetWindowPos.Call(input, 0,
		uintptr(scale(12)), uintptr(scale(12)),
		uintptr(scale(616)), uintptr(scale(int32(uiConf.InputHeightDip))), flags)
	procSetWindowPos.Call(response, 0,
		uintptr(scale(12)), uintptr(scale(152)),
		uintptr(scale(616)), uintptr(scale(int32(uiConf.ResponseHeightDip))), flags)
	procSetWindowPos.Call(status, 0,
		uintptr(scale(12)), uintptr(scale(436)),
		uintptr(scale(416)), uintptr(scale(28)), flags)
	procSetWindowPos.Call(send, 0,
		uintptr(scale(440)), uintptr(scale(436)),
		uintptr(scale(80)), uintptr(scale(28)), flags)
	procSetWindowPos.Call(cancel, 0,
		uintptr(scale(528)), uintptr(scale(436)),
		uintptr(scale(100)), uintptr(scale(28)), flags)
}

// controlSubclass is the shared comctl32 subclass for all composer children.
// subclassID 1 is the editable input (IME tracking + Ctrl+Enter); all controls
// count WM_PAINT callbacks.
func controlSubclass(hwnd uintptr, message uint32, wparam, lparam, subclassID, refData uintptr) uintptr {
	ui := (*UI)(unsafe.Pointer(refData))
	if message == wmPAINT {
		ui.paints++
		r, _, _ := procDefSubclassProc.Call(hwnd, uintptr(message), wparam, lparam)
		return r
	}
	if message == wmNCDESTROY {
		procRemoveWindowSubclass.Call(hwnd, subclassProcPtr, subclassID)
		r, _, _ := procDefSubclassProc.Call(hwnd, uintptr(message), wparam, lparam)
		return r
	}
	if subclassID != 1 {
		r, _, _ := procDefSubclassProc.Call(hwnd, uintptr(message), wparam, lparam)
		return r
	}
	switch message {
	case wmIMESTARTCOMPOSITION:
		ui.onIMEStart(hwnd)
	case wmIMEENDCOMPOSITION:
		ui.onIMEEnd(hwnd)
	case wmIMECOMPOSITION:
		ui.onIMEUpdate()
	case wmKEYDOWN:
		r, _, _ := procGetKeyState.Call(vkCONTROL)
		if uint16(wparam) == vkRETURN && int16(r) < 0 && !ui.composing {
			ui.postSubmit()
			return 0
		}
	}
	r, _, _ := procDefSubclassProc.Call(hwnd, uintptr(message), wparam, lparam)
	return r
}

func subclassControl(hwnd uintptr, ui *UI, isInput bool) bool {
	id := uintptr(2)
	if isInput {
		id = 1
	}
	r, _, _ := procSetWindowSubclass.Call(hwnd, subclassProcPtr, id, uintptr(unsafe.Pointer(ui)))
	return r != 0
}

func registerClasses(instance uintptr) error {
	mascotWndProcPtr = windows.NewCallback(mascotWndProc)
	composerWndProcPtr = windows.NewCallback(composerWndProc)
	subclassProcPtr = windows.NewCallback(controlSubclass)
	mascotName := widep("MascotGoMascot")
	arrow, _, _ := procLoadCursorW.Call(0, idcARROW)
	mascotClass := wndclassw{
		LpfnWndProc:   mascotWndProcPtr,
		HInstance:     instance,
		HCursor:       arrow,
		LpszClassName: mascotName,
	}
	if r, _, _ := procRegisterClassW.Call(uintptr(unsafe.Pointer(&mascotClass))); r == 0 {
		return errString("mascot class registration failed")
	}
	composerName := widep("MascotGoComposer")
	brush, _, _ := procGetSysColorBrush.Call(colorWINDOW)
	composerClass := wndclassw{
		LpfnWndProc:   composerWndProcPtr,
		HInstance:     instance,
		HCursor:       arrow,
		HbrBackground: brush,
		LpszClassName: composerName,
	}
	if r, _, _ := procRegisterClassW.Call(uintptr(unsafe.Pointer(&composerClass))); r == 0 {
		return errString("composer class registration failed")
	}
	return nil
}

func createMascot(ui *UI, instance uintptr) (uintptr, error) {
	var cursor point
	procGetCursorPos.Call(uintptr(unsafe.Pointer(&cursor)))
	monitor, _, _ := procMonitorFromPoint.Call(
		uintptr(uint32(cursor.X))|uintptr(uint32(cursor.Y))<<32, monitorDEFAULTTONEAREST)
	mi := monitorinfo{CbSize: uint32(unsafe.Sizeof(monitorinfo{}))}
	procGetMonitorInfoW.Call(monitor, uintptr(unsafe.Pointer(&mi)))
	dpi, _, _ := procGetDpiForSystem.Call()
	sz := dip(int32(ui.cfg.manifest.Asset.LogicalWidthDip), uint32(dpi))
	x := clamp(cursor.X+16, mi.RcWork.Left, max32(mi.RcWork.Right-sz, mi.RcWork.Left))
	y := clamp(cursor.Y+16, mi.RcWork.Top, max32(mi.RcWork.Bottom-sz, mi.RcWork.Top))
	hwnd, _, _ := procCreateWindowExW.Call(
		wsEXLAYERED|wsEXTOPMOST|wsEXTOOLWINDOW|wsEXNOACTIVATE,
		uintptr(unsafe.Pointer(widep("MascotGoMascot"))),
		uintptr(unsafe.Pointer(widep("mascot"))),
		wsPOPUP,
		uintptr(x), uintptr(y), uintptr(sz), uintptr(sz),
		0, 0, instance,
		uintptr(unsafe.Pointer(ui)),
	)
	if hwnd == 0 {
		return 0, errString("mascot creation failed")
	}
	return hwnd, nil
}
