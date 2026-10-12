import AppKit
import CoreGraphics
import ApplicationServices

let args = CommandLine.arguments
precondition(args.count == 4)
let pid = pid_t(args[1])!
let count = Int(args[3])!
precondition(count > 0 && count <= 100)
precondition(AXIsProcessTrusted())
precondition(NSRunningApplication(processIdentifier: pid)?.executableURL?.lastPathComponent == "psycho")
let key: CGKeyCode
switch args[2] {
case "down": key = 125
case "up": key = 126
default: fatalError("Unsupported key")
}
for _ in 0..<count {
    precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: down)!
        event.postToPid(pid)
    }
    Thread.sleep(forTimeInterval: 0.04)
}
