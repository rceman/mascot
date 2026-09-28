package main

// Native text control helpers: msftedit (RICHEDIT50W) loading, plaintext-mode
// configuration, UTF-16 get/set/append, selection, IME composition handling and
// the shared control subclass used for paint counting + input IME/submit keys.

import (
	"strings"
	"unicode/utf16"
	"unsafe"
)


// loadRichedit initializes OLE on the calling (locked UI) thread and loads the
// system msftedit.dll.
func loadRichedit() (uintptr, error) {
	r, _, _ := procOleInitialize.Call(0)
	// S_OK == 0, S_FALSE == 1 are both success HRESULTs.
	if int32(r) < 0 {
		return 0, errString("OleInitialize failed")
	}
	module, _, _ := procLoadLibraryExW.Call(
		uintptr(unsafe.Pointer(widep("msftedit.dll"))),
		0,
		loadLIBRARYSEARCHSYSTEM32,
	)
	if module == 0 {
		procOleUninitialize.Call()
		return 0, errString("msftedit.dll load failed")
	}
	return module, nil
}

func createEdit(parent, instance uintptr, x, y, width, height int32,
	readOnly bool, charLimit, undoLimit uintptr, font uintptr, controlID uintptr) (uintptr, error) {

	style := uint32(wsCHILD | wsVISIBLE | wsVSCROLL | esMULTILINE | esWANTRETURN | esNOHIDESEL)
	if readOnly {
		style |= esREADONLY
	}
	hwnd, _, _ := procCreateWindowExW.Call(
		0,
		uintptr(unsafe.Pointer(widep("RICHEDIT50W"))),
		0,
		uintptr(style),
		uintptr(x), uintptr(y), uintptr(width), uintptr(height),
		parent,
		controlID,
		instance,
		0,
	)
	if hwnd == 0 {
		return 0, errString("edit control creation failed")
	}
	sendMessage(hwnd, emSETTEXTMODE, tmPLAINTEXT|tmMULTILEVELUNDO|tmMULTICODEPAGE, 0)
	sendMessage(hwnd, emSETTYPOGRAPHYOPTIONS, toADVANCEDTYPOGRAPHY, toADVANCEDTYPOGRAPHY)
	sendMessage(hwnd, emSETLIMITTEXT, charLimit, 0)
	sendMessage(hwnd, emSETUNDOLIMIT, undoLimit, 0)
	sendMessage(hwnd, wmSETFONT, font, 1)
	return hwnd, nil
}

func getText(hwnd uintptr) string {
	var lengthQuery getTextLengthEx // flags 0, codepage 1200
	lengthQuery.Codepage = 1200
	units := sendMessage(hwnd, emGETTEXTLENGTHEX, uintptr(unsafe.Pointer(&lengthQuery)), 0)
	buffer := make([]uint16, int(units)+1)
	query := getTextEx{Cb: uint32(len(buffer) * 2), Codepage: 1200}
	copied := sendMessage(hwnd, emGETTEXTEX, uintptr(unsafe.Pointer(&query)), uintptr(unsafe.Pointer(&buffer[0])))
	if int64(copied) < 0 {
		copied = 0
	}
	if copied > uintptr(len(buffer)) {
		copied = uintptr(len(buffer))
	}
	return normalizeText(string(utf16.Decode(buffer[:copied])))
}

func appendText(hwnd uintptr, value string) {
	value = strings.ReplaceAll(normalizeText(value), "\n", "\r")
	w := wide(value)
	sendMessage(hwnd, emSETSEL, ^uintptr(0), ^uintptr(0))
	sendMessage(hwnd, emREPLACESEL, 0, uintptr(unsafe.Pointer(&w[0])))
	sendMessage(hwnd, emSCROLLCARET, 0, 0)
}

func setText(hwnd uintptr, text string) {
	crlf := strings.ReplaceAll(text, "\n", "\r\n")
	w := wide(crlf)
	options := settextex{Flags: 0, Codepage: 1200}
	sendMessage(hwnd, emSETTEXTEX, uintptr(unsafe.Pointer(&options)), uintptr(unsafe.Pointer(&w[0])))
}

func getSelection(hwnd uintptr) charrange {
	var r charrange
	sendMessage(hwnd, emEXGETSEL, 0, uintptr(unsafe.Pointer(&r)))
	return r
}

func setSelection(hwnd uintptr, min, max int32) {
	r := charrange{Min: min, Max: max}
	sendMessage(hwnd, emEXSETSEL, 0, uintptr(unsafe.Pointer(&r)))
}


func cancelComposition(hwnd uintptr) {
	ctx, _, _ := procImmGetContext.Call(hwnd)
	if ctx != 0 {
		procImmNotifyIME.Call(ctx, niCOMPOSITIONSTR, cpsCANCEL, 0)
		procImmReleaseContext.Call(hwnd, ctx)
	}
}

