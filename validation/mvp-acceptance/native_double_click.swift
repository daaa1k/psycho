import AppKit
import CoreGraphics

let args = CommandLine.arguments
guard args.count == 4, let pid = pid_t(args[1]), pid > 0,
      let x = Double(args[2]), let y = Double(args[3]), x.isFinite, y.isFinite else {
    fatalError("Usage: native-double-click OWNED_PID X Y")
}
precondition(NSRunningApplication(processIdentifier: pid)?.executableURL?.lastPathComponent == "psycho")
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let window = windows.first { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }!
let bounds = CGRect(dictionaryRepresentation: window[kCGWindowBounds as String] as! CFDictionary)!
let point = CGPoint(x: bounds.minX + x, y: bounds.minY + y)
precondition(bounds.contains(point))
for count in 1...2 {
    for kind in [CGEventType.leftMouseDown, .leftMouseUp] {
        let event = CGEvent(mouseEventSource: nil, mouseType: kind, mouseCursorPosition: point, mouseButton: .left)!
        event.flags = []
        event.setIntegerValueField(.mouseEventClickState, value: Int64(count))
        event.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.04)
    }
}
