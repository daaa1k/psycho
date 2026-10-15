import AppKit
import ApplicationServices
import CoreGraphics

func dismissWelcome() -> Bool {
    guard let app = NSWorkspace.shared.runningApplications.first(where: {
        $0.bundleIdentifier == "com.apple.VoiceOverQuickstart"
    }) else { return false }
    func pressOff(_ element: AXUIElement) -> Bool {
        var title: CFTypeRef?
        AXUIElementCopyAttributeValue(element, kAXTitleAttribute as CFString, &title)
        if let title = title as? String,
           ["VoiceOverをオフにする", "Turn Off VoiceOver"].contains(title) {
            return AXUIElementPerformAction(element, kAXPressAction as CFString) == .success
        }
        var children: CFTypeRef?
        AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &children)
        return (children as? [AXUIElement] ?? []).contains(where: pressOff)
    }
    return pressOff(AXUIElementCreateApplication(app.processIdentifier))
}

if NSWorkspace.shared.isVoiceOverEnabled && !dismissWelcome() {
    let source = CGEventSource(stateID: .combinedSessionState)
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: source, virtualKey: 96, keyDown: down)!
        event.flags = [.maskCommand, .maskSecondaryFn]
        event.post(tap: .cghidEventTap)
    }
}
for _ in 0..<30 {
    if !NSWorkspace.shared.isVoiceOverEnabled { exit(0) }
    _ = dismissWelcome()
    Thread.sleep(forTimeInterval: 0.2)
}
fatalError("VoiceOver did not stop")
