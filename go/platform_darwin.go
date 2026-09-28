package main

// Go↔AppKit bridge. All Objective-C lives in mascot_darwin.m; this file holds
// the cgo wrappers used by ui_darwin.go plus the //export callbacks invoked on
// the main thread by the dispatch pumps and native delegates.

/*
#cgo CFLAGS: -fobjc-arc
#cgo LDFLAGS: -framework Cocoa -framework Carbon

#include <stdlib.h>
#include "mascot_darwin.h"
*/
import "C"

import (
	"fmt"
	"os"
	"unsafe"
)

type mascotHandles struct {
	window   unsafe.Pointer
	input    unsafe.Pointer
	response unsafe.Pointer
	status   unsafe.Pointer
	send     unsafe.Pointer
	cancel   unsafe.Pointer
}

func mascotAppInit() bool {
	return C.mascot_app_init() != 0
}

func mascotCreateMascot(rgba []byte, srcW, srcH int, dip float64) unsafe.Pointer {
	if len(rgba) == 0 {
		return nil
	}
	p := C.mascot_create_mascot((*C.uchar)(unsafe.Pointer(&rgba[0])),
		C.int(srcW), C.int(srcH), C.double(dip))
	return unsafe.Pointer(p)
}

func mascotOrderFront(panel unsafe.Pointer) {
	C.mascot_order_front(panel)
}

func composerHandlesC(c *composer) C.MascotComposer {
	return C.MascotComposer{
		window:   c.window,
		input:    c.input,
		response: c.response,
		status:   c.status,
		send:     c.send,
		cancel:   c.cancel,
	}
}

func mascotCreateComposer(panel unsafe.Pointer, width, height, inputH, responseH, margin float64) mascotHandles {
	h := C.mascot_create_composer(panel, C.double(width), C.double(height),
		C.double(inputH), C.double(responseH), C.double(margin))
	return mascotHandles{
		window:   h.window,
		input:    h.input,
		response: h.response,
		status:   h.status,
		send:     h.send,
		cancel:   h.cancel,
	}
}

func mascotShowComposer(c *composer) {
	C.mascot_show_composer(composerHandlesC(c))
}

func mascotHideComposer(c *composer) {
	C.mascot_hide_composer(composerHandlesC(c))
}

func mascotComposerVisible(c *composer) bool {
	return C.mascot_composer_visible(composerHandlesC(c)) != 0
}

func mascotTextGet(tv unsafe.Pointer) string {
	cstr := C.mascot_text_get(tv)
	if cstr == nil {
		return ""
	}
	defer C.free(unsafe.Pointer(cstr))
	return C.GoString(cstr)
}

func mascotTextSet(tv unsafe.Pointer, value string) {
	cstr := C.CString(value)
	defer C.free(unsafe.Pointer(cstr))
	C.mascot_text_set(tv, cstr)
}

func mascotTextAppend(tv unsafe.Pointer, value string) {
	cstr := C.CString(value)
	defer C.free(unsafe.Pointer(cstr))
	C.mascot_text_append(tv, cstr)
}

func mascotSelectionGet(tv unsafe.Pointer) (int64, int64) {
	var loc, length C.long
	C.mascot_selection_get(tv, &loc, &length)
	return int64(loc), int64(length)
}

func mascotSelectionSet(tv unsafe.Pointer, location, length int32) {
	C.mascot_selection_set(tv, C.long(location), C.long(length))
}

func mascotStatusSet(field unsafe.Pointer, value string) {
	cstr := C.CString(value)
	defer C.free(unsafe.Pointer(cstr))
	C.mascot_status_set(field, cstr)
}

func mascotHasMarked(tv unsafe.Pointer) bool {
	return C.mascot_has_marked(tv) != 0
}

func mascotUnmark(tv unsafe.Pointer) {
	C.mascot_unmark(tv)
}

func mascotIMESelect(id string) int {
	cstr := C.CString(id)
	defer C.free(unsafe.Pointer(cstr))
	return int(C.mascot_ime_select(cstr))
}

func mascotIMEMark(text string) {
	cstr := C.CString(text)
	defer C.free(unsafe.Pointer(cstr))
	C.mascot_ime_mark(cstr)
}

func mascotIMEInsert(text string) {
	cstr := C.CString(text)
	defer C.free(unsafe.Pointer(cstr))
	C.mascot_ime_insert(cstr)
}

func mascotIMEDiscard() {
	C.mascot_ime_discard()
}

func mascotFocusInfo() int {
	return int(C.mascot_focus_info())
}

func mascotRegisterHotkeys() bool {
	return C.mascot_register_hotkeys() != 0
}

func mascotUnregisterHotkeys() {
	C.mascot_unregister_hotkeys()
}

func mascotRun() {
	C.mascot_run()
}

func mascotTerminate() {
	C.mascot_terminate()
}

func mascotTeardownComposer(c *composer) {
	C.mascot_teardown(composerHandlesC(c), nil)
}

func mascotTeardownPanel(panel unsafe.Pointer) {
	var empty C.MascotComposer
	C.mascot_teardown(empty, panel)
}

func (ui *UI) postProviderWake() {
	C.mascot_post_ui()
}

func (ui *UI) postControlWake() {
	C.mascot_post_control()
}

// ------------------------------------------------------------- Go→C exports

//export goDispatchUIEvents
func goDispatchUIEvents() {
	if theUI != nil {
		theUI.dispatchProviderEvents()
	}
}

//export goDispatchControl
func goDispatchControl() {
	if theUI != nil {
		theUI.dispatchControl()
	}
}

//export goUIHotkey
func goUIHotkey(id C.int) {
	ui := theUI
	if ui == nil {
		return
	}
	switch id {
	case 1:
		if ui.composerVisible() {
			ui.hideComposer()
		} else if err := ui.showComposer(); err != nil {
			fmt.Fprintln(os.Stderr, "hotkey show:", err)
		}
	case 2:
		if err := ui.cancelRequest(); err != nil {
			fmt.Fprintln(os.Stderr, "hotkey cancel:", err)
		}
	}
}

//export goUISubmit
func goUISubmit() {
	if theUI != nil {
		if err := theUI.submit(); err != nil {
			fmt.Fprintln(os.Stderr, "submit:", err)
		}
	}
}

//export goUIMarked
func goUIMarked(hasMarked C.int) {
	if theUI != nil {
		theUI.onMarkedChanged(hasMarked != 0)
	}
}

//export goUIUnmark
func goUIUnmark() {
	if theUI != nil {
		theUI.onUnmark()
	}
}

//export goUISendAction
func goUISendAction() {
	if theUI != nil {
		if err := theUI.submit(); err != nil {
			fmt.Fprintln(os.Stderr, "send:", err)
		}
	}
}

//export goUICancelAction
func goUICancelAction() {
	if theUI != nil {
		if err := theUI.cancelRequest(); err != nil {
			fmt.Fprintln(os.Stderr, "cancel:", err)
		}
	}
}

//export goUIWindowShouldClose
func goUIWindowShouldClose() C.int {
	if theUI != nil {
		theUI.hideComposer()
	}
	return 0
}

//export goUIShouldInsert
func goUIShouldInsert(affectedLoc, affectedLen C.long, replacement *C.char) C.int {
	if theUI == nil {
		return 1
	}
	if theUI.shouldInsert(int64(affectedLoc), int64(affectedLen), C.GoString(replacement)) {
		return 1
	}
	return 0
}

//export goUIPaint
func goUIPaint() {
	if theUI != nil {
		theUI.paints++
	}
}

//export goUIPresent
func goUIPresent() {
	if theUI != nil {
		theUI.presents++
	}
}
