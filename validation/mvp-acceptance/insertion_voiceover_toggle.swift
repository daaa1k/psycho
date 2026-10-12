import Cocoa
import CoreGraphics

guard let pid = Int32(CommandLine.arguments[1]),
      NSWorkspace.shared.frontmostApplication?.processIdentifier == pid,
      NSRunningApplication(processIdentifier: pid)?.localizedName == "psycho" else {
    fatalError("Owned scratch PSYCHO must be foreground")
}
let source = CGEventSource(stateID: .combinedSessionState)
for down in [true, false] {
    let event = CGEvent(keyboardEventSource: source, virtualKey: 96, keyDown: down)!
    event.flags = [.maskCommand, .maskSecondaryFn]
    event.post(tap: .cghidEventTap)
}
Thread.sleep(forTimeInterval: 1)
print(NSWorkspace.shared.isVoiceOverEnabled ? "true" : "false")
