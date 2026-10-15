import AppKit

let args = CommandLine.arguments
let pid = Int32(args[1])!
let app = NSRunningApplication(processIdentifier: pid)!
precondition(app.executableURL?.lastPathComponent == "psycho")
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let window = windows.first { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }!
let bounds = CGRect(dictionaryRepresentation: window[kCGWindowBounds as String] as! CFDictionary)!
let point = CGPoint(x: bounds.minX + Double(args[2])!, y: bounds.minY + Double(args[3])!)
CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left)!.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 0.3)
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let event = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: Int32(args[4])!, wheel2: 0, wheel3: 0)!
event.location = point
event.post(tap: .cghidEventTap)
Thread.sleep(forTimeInterval: 0.3)
