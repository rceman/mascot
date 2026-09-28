// Dump on-screen window bounds for a given PID via CGWindowList (no
// Screen Recording permission required for geometry metadata).
// Usage: macos_window_bounds <pid>
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 2, let pid = Int32(CommandLine.arguments[1]) else {
    FileHandle.standardError.write("usage: macos_window_bounds <pid>\n".data(using: .utf8)!)
    exit(64)
}

guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements],
                                            CGWindowID(0)) as? [[String: Any]] else {
    exit(1)
}
for win in list {
    guard let owner = win[kCGWindowOwnerPID as String] as? Int32, owner == pid else { continue }
    let bounds = win[kCGWindowBounds as String] as? [String: Any] ?? [:]
    let layer = win[kCGWindowLayer as String] as? Int ?? -1
    let name = win[kCGWindowName as String] as? String ?? ""
    let id = win[kCGWindowNumber as String] as? Int ?? 0
    let row: [String: Any] = [
        "window_id": id,
        "layer": layer,
        "name": name,
        "x": bounds["X"] ?? 0,
        "y": bounds["Y"] ?? 0,
        "w": bounds["Width"] ?? 0,
        "h": bounds["Height"] ?? 0,
    ]
    if let data = try? JSONSerialization.data(withJSONObject: row),
       let line = String(data: data, encoding: .utf8) {
        print(line)
    }
}
