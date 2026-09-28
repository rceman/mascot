package main

/*
#include <mach/mach_time.h>

static uint64_t mascot_mach_time(void) { return mach_absolute_time(); }
static int64_t mascot_mach_freq(void) {
	mach_timebase_info_data_t info;
	if (mach_timebase_info(&info) != 0 || info.numer == 0) {
		return -1;
	}
	return (int64_t)(1e9 * (double)info.denom / (double)info.numer);
}
*/
import "C"

// The macOS common clock is mach_absolute_time; emit_qpc/receipt_qpc keep the
// same wire names and share this domain with the darwin fixture build.
func qpc() int64 {
	return int64(C.mascot_mach_time())
}

func qpcFrequency() int64 {
	v := int64(C.mascot_mach_freq())
	if v <= 0 {
		panic("mach_timebase_info failed")
	}
	return v
}
