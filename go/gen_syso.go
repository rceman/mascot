//go:build ignore

// gen_syso converts an SDK rc.exe-produced .res file into the minimal COFF
// object (.syso) that the Go linker embeds as the PE .rsrc section.
//
// The MSVC cvtres.exe converter additionally emits a .debug$S section plus
// @comp.id/@feat.00 absolute symbols whose negative section numbers the Go
// linker rejects ("sectnum < 0"), so this tool emits the equivalent
// .rsrc$01/.rsrc$02 sections directly. This is the same resource tree layout
// (RT_MANIFEST id 1, language from the .res header) that cvtres produces.
//
// Usage: go run gen_syso.go app.res rsrc_windows_amd64.syso
package main

import (
	"encoding/binary"
	"fmt"
	"os"
)

const (
	imageFileMachineAMD64       = 0x8664
	imageSymClassStatic         = 3
	imageRelAMD64Addr32NB       = 3
	imageScnCntInitializedData  = 0x40
	imageScnMemRead             = 0x40000000
	imageScnMemWrite            = 0x80000000
	rtManifest                  = 24
	resourceID                  = 1
)

func main() {
	if len(os.Args) != 3 {
		fmt.Fprintln(os.Stderr, "usage: gen_syso <input.res> <output.syso>")
		os.Exit(64)
	}
	res, err := os.ReadFile(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(64)
	}
	lang, data, err := parseRes(res)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(64)
	}
	if err := os.WriteFile(os.Args[2], buildCOFF(lang, data), 0o644); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(64)
	}
}

// parseRes extracts the manifest resource record's language id and payload,
// skipping rc.exe's leading empty sentinel record.
func parseRes(res []byte) (lang uint16, data []byte, err error) {
	for {
		if len(res) < 32 {
			return 0, nil, fmt.Errorf("res too short: %d", len(res))
		}
		if binary.LittleEndian.Uint32(res[0:]) != 0 {
			break
		}
		// Empty record: DataSize=0, HeaderSize=32 -> advance past it.
		res = res[32:]
	}
	dataSize := binary.LittleEndian.Uint32(res[0:])
	headerSize := binary.LittleEndian.Uint32(res[4:])
	pos := 8
	readOrdinal := func() (uint16, error) {
		if pos+4 > len(res) {
			return 0, fmt.Errorf("res truncated at %d", pos)
		}
		if binary.LittleEndian.Uint16(res[pos:]) != 0xFFFF {
			return 0, fmt.Errorf("non-ordinal res entry at %d", pos)
		}
		v := binary.LittleEndian.Uint16(res[pos+2:])
		pos += 4
		return v, nil
	}
	typ, err := readOrdinal()
	if err != nil {
		return 0, nil, err
	}
	name, err := readOrdinal()
	if err != nil {
		return 0, nil, err
	}
	if typ != rtManifest || name != resourceID {
		return 0, nil, fmt.Errorf("unexpected res entry type=%d name=%d", typ, name)
	}
	if pos+16 > len(res) {
		return 0, nil, fmt.Errorf("res truncated at %d", pos)
	}
	// DataVersion(4) MemoryFlags(2) LanguageId(2) Version(4) Characteristics(4)
	lang = binary.LittleEndian.Uint16(res[pos+6:])
	pos += 16
	if uint32(pos) != headerSize {
		return 0, nil, fmt.Errorf("unexpected res header size %d", headerSize)
	}
	if uint32(pos)+dataSize > uint32(len(res)) {
		return 0, nil, fmt.Errorf("res truncated: need %d have %d", dataSize, len(res)-pos)
	}
	return lang, res[pos : uint32(pos)+dataSize], nil
}

func u32(b []byte, off int, v uint32) { binary.LittleEndian.PutUint32(b[off:], v) }
func u16(b []byte, off int, v uint16) { binary.LittleEndian.PutUint16(b[off:], v) }

// buildCOFF emits an amd64 COFF object with .rsrc$01 (directory tree + data
// entry) and .rsrc$02 (payload) sections plus one ADDR32NB relocation wiring
// DataRVA to the payload, matching the cvtres layout without debug sections.
func buildCOFF(lang uint16, payload []byte) []byte {
	// .rsrc$01: 88 bytes = 3 directory headers (16+8 entry each) + 16-byte
	// IMAGE_RESOURCE_DATA_ENTRY at offset 72.
	s1 := make([]byte, 88)
	u16(s1, 14, 1)          // root: one id entry
	u32(s1, 16, rtManifest) // entry name = RT_MANIFEST
	u32(s1, 20, 0x80000018) // -> subdir at 24
	u16(s1, 38, 1)          // id dir: one id entry
	u32(s1, 40, resourceID) // entry name = 1
	u32(s1, 44, 0x80000030) // -> subdir at 48
	u16(s1, 62, 1)          // lang dir: one id entry
	u32(s1, 64, uint32(lang))
	u32(s1, 68, 0x48) // -> data entry at 72 (leaf)
	u32(s1, 72, 0)    // DataRVA: patched by relocation
	u32(s1, 76, uint32(len(payload)))
	u32(s1, 80, 0) // CodePage
	u32(s1, 84, 0) // Reserved

	const (
		sectHdrSize = 40
		fileHdrSize = 20
		s1Off       = fileHdrSize + 2*sectHdrSize
	)
	relocOff := s1Off + len(s1)
	s2Off := relocOff + 10
	symOff := s2Off + len(payload)
	const nsyms = 5 // two section syms with aux records + $R000000
	strOff := symOff + nsyms*18

	out := make([]byte, strOff+4)
	// file header
	u16(out, 0, imageFileMachineAMD64)
	u16(out, 2, 2)
	u32(out, 8, uint32(symOff))
	u32(out, 12, nsyms)

	// section headers (IMAGE_SECTION_HEADER, 40 bytes each)
	chars := uint32(imageScnCntInitializedData | imageScnMemRead | imageScnMemWrite)
	copy(out[fileHdrSize:], ".rsrc$01")             // Name[8]
	u32(out, fileHdrSize+16, uint32(len(s1)))       // SizeOfRawData
	u32(out, fileHdrSize+20, uint32(s1Off))         // PointerToRawData
	u32(out, fileHdrSize+24, uint32(relocOff))      // PointerToRelocations
	u16(out, fileHdrSize+32, 1)                     // NumberOfRelocations
	u32(out, fileHdrSize+36, chars)                 // Characteristics
	s2 := fileHdrSize + sectHdrSize
	copy(out[s2:], ".rsrc$02")
	u32(out, s2+16, uint32(len(payload)))
	u32(out, s2+20, uint32(s2Off))
	u32(out, s2+36, chars)

	copy(out[s1Off:], s1)
	// relocation: .rsrc$01 offset 72 (DataRVA) -> symbol index 4 ($R000000), ADDR32NB
	u32(out, relocOff, 72)
	u32(out, relocOff+4, 4)
	u16(out, relocOff+8, imageRelAMD64Addr32NB)
	copy(out[s2Off:], payload)

	// symbols
	sym := func(idx int, name string, value uint32, sectnum int16, aux uint8) {
		o := symOff + idx*18
		if len(name) <= 8 {
			copy(out[o:], name)
		}
		u32(out, o+8, value)
		binary.LittleEndian.PutUint16(out[o+12:], uint16(sectnum))
		out[o+17] = aux
	}
	sym(0, ".rsrc$01", 0, 1, 1)
	// aux section record for .rsrc$01
	aux := symOff + 18
	u32(out, aux, uint32(len(s1)))
	u16(out, aux+4, 1)
	sym(2, ".rsrc$02", 0, 2, 1)
	aux2 := symOff + 3*18
	u32(out, aux2, uint32(len(payload)))
	sym(4, "$R000000", 0, 2, 0)
	// mark symbol storage classes as STATIC
	for _, i := range []int{0, 2, 4} {
		out[symOff+i*18+16] = imageSymClassStatic
	}
	// string table size (empty)
	u32(out, strOff, 4)
	return out
}
