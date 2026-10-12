import AppKit
import CoreGraphics

let args = CommandLine.arguments
precondition(args.count == 5)
let pid = pid_t(args[1])!
precondition(NSRunningApplication(processIdentifier: pid)?.executableURL?.lastPathComponent == "psycho")
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let owned = windows.first { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }!
let bounds = CGRect(dictionaryRepresentation: owned[kCGWindowBounds as String] as! CFDictionary)!
let point = CGPoint(x: bounds.minX + Double(args[3])!, y: bounds.minY + Double(args[4])!)
let kind: CGEventType
switch args[2] {
case "down": kind = .leftMouseDown
case "drag": kind = .leftMouseDragged
case "move": kind = .mouseMoved
case "up": kind = .leftMouseUp
case "right": kind = .rightMouseDown
case "right-up": kind = .rightMouseUp
default: fatalError("Unsupported pointer event")
}
let event = CGEvent(mouseEventSource: nil, mouseType: kind, mouseCursorPosition: point,
                    mouseButton: args[2].hasPrefix("right") ? .right : .left)!
event.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 0.15)
