package main

// DIP helpers exist on darwin only so the shared metric tests compile; Cocoa
// geometry is already expressed in points (device-independent).

func dip(value int32, dpi uint32) int32 {
	return (value*int32(dpi) + 48) / 96
}

func fontHeight(dpi uint32) int32 {
	return -dip(16, dpi)
}
