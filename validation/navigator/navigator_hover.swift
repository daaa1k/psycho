import AppKit
import CoreGraphics

let args = CommandLine.arguments
precondition(args.count == 4)
let pid = pid_t(args[1])!
precondition(NSRunningApplication(processIdentifier: pid)?.executableURL?.lastPathComponent == "psycho")
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let owned = windows.first {
    ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0
}!
let bounds = CGRect(dictionaryRepresentation: owned[kCGWindowBounds as String] as! CFDictionary)!
let point = CGPoint(x: bounds.minX + Double(args[2])!, y: bounds.minY + Double(args[3])!)
let move = CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left)!
move.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 1.5)
