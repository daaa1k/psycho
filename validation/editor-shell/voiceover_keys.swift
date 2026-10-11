import AppKit
import CoreGraphics

guard CommandLine.arguments.count == 3,
      let pid = Int32(CommandLine.arguments[1]),
      NSWorkspace.shared.frontmostApplication?.processIdentifier == pid,
      NSRunningApplication(processIdentifier: pid)?.localizedName == "psycho" else {
    fatalError("Owned scratch psycho must be foreground")
}
let copying = CommandLine.arguments[2] == "copy"
let toggling = CommandLine.arguments[2] == "toggle"
let caption = CommandLine.arguments[2] == "caption"
let source = CGEventSource(stateID: .combinedSessionState)
var flags = CGEventFlags()
let modifiers: [(CGKeyCode, CGEventFlags)] = toggling ? [(55, .maskCommand)] : caption
    ? [(59, .maskControl), (58, .maskAlternate), (55, .maskCommand)] : copying
    ? [(59, .maskControl), (58, .maskAlternate), (56, .maskShift)]
    : [(59, .maskControl), (58, .maskAlternate)]
for (key, flag) in modifiers {
    flags.insert(flag)
    let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: true)!
    event.type = .flagsChanged
    event.flags = flags
    event.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.05)
}
for down in [true, false] {
    let event = CGEvent(keyboardEventSource: source, virtualKey: toggling ? 96 : caption ? 109 : copying ? 8 : 124, keyDown: down)!
    event.flags = toggling || caption ? flags.union(.maskSecondaryFn) : flags
    event.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.05)
}
for (key, flag) in modifiers.reversed() {
    flags.remove(flag)
    let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: false)!
    event.type = .flagsChanged
    event.flags = flags
    event.post(tap: .cghidEventTap)
}
Thread.sleep(forTimeInterval: 0.4)
