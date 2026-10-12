import AppKit
import CoreGraphics

let args = CommandLine.arguments
precondition(args.count == 3)
let pid = pid_t(args[1])!
precondition(NSRunningApplication(processIdentifier: pid)?.executableURL?.lastPathComponent == "psycho")
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let owned = windows.first {
    ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0
}!
let bounds = CGRect(dictionaryRepresentation: owned[kCGWindowBounds as String] as! CFDictionary)!
let point = CGPoint(x: bounds.minX + 80, y: bounds.minY + 500)
let move = CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left)!
move.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 0.2)
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let event = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: Int32(args[2])!, wheel2: 0, wheel3: 0)!
event.location = point
event.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 0.4)
