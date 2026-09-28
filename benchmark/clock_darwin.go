//go:build darwin

package main

// The Windows fixture uses QueryPerformanceCounter. The macOS native
// equivalent is mach_absolute_time plus mach_timebase_info; emit_qpc and
// qpc_frequency keep the same wire names and decimal encoding, only the
// platform clock domain changes. This build requires CGO_ENABLED=1.

/*
#include <mach/mach_time.h>
*/
import "C"

func qpc() int64 {
	return int64(C.mach_absolute_time())
}

func frequency() int64 {
	var info C.mach_timebase_info_data_t
	if C.mach_timebase_info(&info) != 0 || info.numer == 0 {
		panic("mach_timebase_info failed")
	}
	return int64(1e9 * float64(info.denom) / float64(info.numer))
}
