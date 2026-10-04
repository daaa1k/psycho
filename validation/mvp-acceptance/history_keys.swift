import AppKit
import CoreGraphics
import ApplicationServices
import Darwin
let a = CommandLine.arguments
precondition(a.count == 4, "usage: history_keys PID paste|undo|redo COUNT")
let pid = pid_t(a[1])!
let count = Int(a[3])!
precondition(count > 0 && count <= 1001)
precondition(AXIsProcessTrusted())
precondition(NSRunningApplication(processIdentifier: pid)?.localizedName == "psycho")
let app = AXUIElementCreateApplication(pid)
let key: CGKeyCode
switch a[2] {
case "paste": key = 9
case "undo": key = 6
case "redo": key = 6
default: fatalError("Unsupported key")
}
for index in 0..<count {
    var frontmost: CFTypeRef?
    guard AXUIElementCopyAttributeValue(app, kAXFrontmostAttribute as CFString, &frontmost) == .success,
          (frontmost as? Bool) == true else {
        fputs("Scratch app lost foreground focus after \(index) keys; no more keys dispatched\n", stderr)
        exit(2)
    }
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: down)!
        event.flags = a[2] == "redo" ? [.maskCommand, .maskShift] : .maskCommand
        event.postToPid(pid)
    }
    Thread.sleep(forTimeInterval: 0.025)
}
