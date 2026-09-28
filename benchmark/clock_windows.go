//go:build windows

package main

import (
	"syscall"
	"unsafe"
)

var kernel32 = syscall.NewLazyDLL("kernel32.dll")
var queryCounter = kernel32.NewProc("QueryPerformanceCounter")
var queryFrequency = kernel32.NewProc("QueryPerformanceFrequency")

func qpc() int64 {
	var value int64
	ok, _, _ := queryCounter.Call(uintptr(unsafe.Pointer(&value)))
	if ok == 0 {
		panic("QueryPerformanceCounter failed")
	}
	return value
}

func frequency() int64 {
	var value int64
	ok, _, _ := queryFrequency.Call(uintptr(unsafe.Pointer(&value)))
	if ok == 0 || value <= 0 {
		panic("QueryPerformanceFrequency failed")
	}
	return value
}
