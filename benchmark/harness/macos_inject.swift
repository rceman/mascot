// macOS Stage B interaction injector + own-window screenshot tool.
// Uses CGEventPost (requires Accessibility) and CGWindowListCreateImage
// (own-window capture does not require Screen Recording).
//
// Usage:
//   macos_inject chord ctrl,alt 49        -> key down+up of keycode with flags
//   macos_inject keydown ctrl,alt 49      -> key down only
//   macos_inject keyup ctrl,alt 49        -> key up only
//   macos_inject type <utf8-text>         -> type unicode text keystrokes
//   macos_inject click x y                -> left click at screen point
//   macos_inject drag x1 y1 x2 y2 [steps] -> left-button drag
//   macos_inject clickthrough x y         -> click, return event to caller only
//   macos_inject shot <window_id> <out.png>
//   macos_inject ime list                 -> enabled text input sources
//   macos_inject ime select <id>          -> select input source by id substring

import ApplicationServices
import CoreGraphics
import Foundation
import Carbon

func err(_ msg: String) -> Never {
    FileHandle.standardError.write((msg + "\n").data(using: .utf8)!)
    exit(1)
}

func keyCodeFlags(_ names: [String]) -> CGEventFlags {
    var flags = CGEventFlags()
    for n in names {
        switch n.lowercased() {
        case "ctrl", "control": flags.insert(.maskControl)
        case "alt", "option": flags.insert(.maskAlternate)
        case "shift": flags.insert(.maskShift)
        case "cmd", "command", "meta": flags.insert(.maskCommand)
        default: err("unknown flag \(n)")
        }
    }
    return flags
}

func postKey(_ code: CGKeyCode, _ flags: CGEventFlags, _ down: Bool) {
    guard let ev = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
        err("key event creation failed")
    }
    ev.flags = flags
    ev.post(tap: .cghidEventTap)
}

func postChord(_ flagNames: [String], _ code: CGKeyCode, down: Bool, up: Bool) {
    // Mirror what a real keyboard produces: modifiers arrive as flags on the
    // key event itself; posting just the key event with flags is sufficient
    // for RegisterEventHotKey and AppKit key handling.
    if down { postKey(code, keyCodeFlags(flagNames), true) }
    usleep(30_000)
    if up { postKey(code, keyCodeFlags(flagNames), false) }
}

// US keyboard layout keycode map for ASCII characters; used to drive real
// input-method composition (unicode-injected events bypass the IME).
let keycodeMap: [Character: CGKeyCode] = [
    "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8,
    "v": 9, "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17,
    "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25,
    "7": 26, "-": 27, "8": 28, "0": 29, "]": 30, "o": 31, "u": 32, "[": 33,
    "i": 34, "p": 35, "l": 37, "j": 38, "'": 39, "k": 40, ";": 41, "\\": 42,
    ",": 43, "/": 44, "n": 45, "m": 46, ".": 47, "`": 50, " ": 49,
]

func postKeycodes(_ text: String, _ flags: CGEventFlags) {
    for ch in text {
        guard let code = keycodeMap[ch] else {
            err("no keycode mapping for '\(ch)'")
        }
        postKey(code, flags, true)
        usleep(12_000)
        postKey(code, flags, false)
        usleep(12_000)
    }
}

func postUnicodeText(_ text: String) {
    for scalar in text.unicodeScalars {
        var units = Array(String(scalar).utf16)
        // Post the whole UTF-16 sequence (incl. surrogate pairs) in a single
        // event; split units are invalid input to the text system.
        guard let down = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: true),
              let up = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: false) else {
            err("unicode event creation failed")
        }
        units.withUnsafeMutableBufferPointer { buf in
            down.keyboardSetUnicodeString(stringLength: buf.count,
                                          unicodeString: buf.baseAddress)
            up.keyboardSetUnicodeString(stringLength: buf.count,
                                        unicodeString: buf.baseAddress)
        }
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
        usleep(14_000)
        units.removeAll()
    }
}

func postClick(_ x: Double, _ y: Double) {
    let p = CGPoint(x: x, y: y)
    guard let down = CGEvent(mouseEventSource: nil, mouseType: .leftMouseDown,
                             mouseCursorPosition: p, mouseButton: .left),
          let up = CGEvent(mouseEventSource: nil, mouseType: .leftMouseUp,
                           mouseCursorPosition: p, mouseButton: .left) else {
        err("mouse event creation failed")
    }
    down.post(tap: .cghidEventTap)
    usleep(30_000)
    up.post(tap: .cghidEventTap)
}

func postDrag(_ x1: Double, _ y1: Double, _ x2: Double, _ y2: Double, _ steps: Int) {
    let p1 = CGPoint(x: x1, y: y1)
    guard let down = CGEvent(mouseEventSource: nil, mouseType: .leftMouseDown,
                             mouseCursorPosition: p1, mouseButton: .left) else {
        err("drag down failed")
    }
    down.post(tap: .cghidEventTap)
    usleep(50_000)
    for i in 1...max(steps, 1) {
        let t = Double(i) / Double(steps)
        let p = CGPoint(x: x1 + (x2 - x1) * t, y: y1 + (y2 - y1) * t)
        guard let ev = CGEvent(mouseEventSource: nil, mouseType: .leftMouseDragged,
                               mouseCursorPosition: p, mouseButton: .left) else {
            err("drag move failed")
        }
        ev.post(tap: .cghidEventTap)
        usleep(15_000)
    }
    guard let up = CGEvent(mouseEventSource: nil, mouseType: .leftMouseUp,
                           mouseCursorPosition: CGPoint(x: x2, y: y2), mouseButton: .left) else {
        err("drag up failed")
    }
    up.post(tap: .cghidEventTap)
}

func imeProp(_ source: TISInputSource, _ key: CFString) -> String? {
    guard let ptr = TISGetInputSourceProperty(source, key) else { return nil }
    let cfStr = Unmanaged<CFString>.fromOpaque(ptr).takeUnretainedValue()
    return cfStr as String
}

func imeEnabled(_ source: TISInputSource) -> Bool {
    guard let ptr = TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled)
    else { return false }
    let cf = Unmanaged<CFBoolean>.fromOpaque(ptr).takeUnretainedValue()
    return CFBooleanGetValue(cf)
}

func imeList() {
    guard let arr = TISCreateInputSourceList([:] as CFDictionary, true)?
        .takeRetainedValue() as? [TISInputSource] else {
        err("TISCreateInputSourceList failed")
    }
    for s in arr {
        let id = imeProp(s, kTISPropertyInputSourceID) ?? "?"
        let name = imeProp(s, kTISPropertyLocalizedName) ?? "?"
        let en = imeEnabled(s) ? "enabled" : "disabled"
        print("\(id) | \(en) | \(name)")
    }
}

func imeSelect(_ needle: String) {
    guard let arr = TISCreateInputSourceList([:] as CFDictionary, true)?
        .takeRetainedValue() as? [TISInputSource] else {
        err("TISCreateInputSourceList failed")
    }
    var best: TISInputSource? = nil
    for s in arr {
        let id = imeProp(s, kTISPropertyInputSourceID) ?? ""
        if id == needle || (best == nil && id.lowercased().contains(needle.lowercased())) {
            best = s
            if id == needle {
                print("selecting \(id)")
                break
            }
        }
    }
    guard let target = best else { err("input source not found: \(needle)") }
    print("selecting \(imeProp(target, kTISPropertyInputSourceID) ?? "?")")
    let status = TISSelectInputSource(target)
    if status != noErr { err("TISSelectInputSource -> \(status)") }
    print("selected")
}

let args = CommandLine.arguments
guard args.count >= 2 else { err("missing command") }
switch args[1] {
case "chord":
    guard args.count == 4 else { err("chord flags code") }
    postChord(args[2].split(separator: ",").map(String.init),
              CGKeyCode(args[3]) ?? 0, down: true, up: true)
case "keydown":
    guard args.count == 4 else { err("keydown flags code") }
    postChord(args[2].split(separator: ",").map(String.init),
              CGKeyCode(args[3]) ?? 0, down: true, up: false)
case "keyup":
    guard args.count == 4 else { err("keyup flags code") }
    postChord(args[2].split(separator: ",").map(String.init),
              CGKeyCode(args[3]) ?? 0, down: false, up: true)
case "type":
    guard args.count == 3 else { err("type text") }
    postUnicodeText(args[2])
case "keytype":
    guard args.count == 3 else { err("keytype text") }
    postKeycodes(args[3 - 1], CGEventFlags())
case "click":
    guard args.count == 4 else { err("click x y") }
    postClick(Double(args[2])!, Double(args[3])!)
case "clickthrough":
    guard args.count == 4 else { err("clickthrough x y") }
    postClick(Double(args[2])!, Double(args[3])!)
case "drag":
    guard args.count >= 5 else { err("drag x1 y1 x2 y2 [steps]") }
    postDrag(Double(args[2])!, Double(args[3])!, Double(args[4])!, Double(args[5])!,
             args.count > 6 ? Int(args[6])! : 12)
case "shot":
    err("shot moved to macos_shot (CGWindowListCreateImage is C-only on this SDK)")
case "ime":
    if args.count == 3 && args[2] == "list" {
        imeList()
    } else if args.count == 4 && args[2] == "select" {
        imeSelect(args[3])
    } else {
        err("ime list | ime select <id>")
    }
default:
    err("unknown command \(args[1])")
}
