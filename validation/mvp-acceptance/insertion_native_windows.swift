import Cocoa
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
let menus = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid && ($0[kCGWindowLayer as String] as? Int ?? 0) > 0 }
let data = try JSONSerialization.data(withJSONObject: menus, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(data)
