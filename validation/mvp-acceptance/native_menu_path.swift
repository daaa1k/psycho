import Cocoa
import ApplicationServices

func value(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &result) == .success else { return nil }
    return result
}
func matches(_ element: AXUIElement, _ title: String) -> [AXUIElement] {
    var result = value(element, kAXTitleAttribute) as? String == title ? [element] : []
    for child in value(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
        result += matches(child, title)
    }
    return result
}
guard CommandLine.arguments.count >= 3, let pid = Int32(CommandLine.arguments[1]),
      NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
    fatalError("Usage: native-menu-path FOREGROUND_OWNED_PID CATEGORY PARENT ITEM")
}
let app = AXUIElementCreateApplication(pid)
var menuBar: CFTypeRef?
guard AXUIElementCopyAttributeValue(app, kAXMenuBarAttribute as CFString, &menuBar) == .success,
      let menuBar = menuBar else { fatalError("No AX menu bar") }
var current = unsafeBitCast(menuBar, to: AXUIElement.self)
for title in CommandLine.arguments.dropFirst(2) {
    let found = matches(current, title)
    guard found.count == 1, value(found[0], kAXEnabledAttribute) as? Bool == true else {
        fatalError("Menu path component is missing, ambiguous, or disabled: \(title)")
    }
    current = found[0]
    guard AXUIElementPerformAction(current, kAXPressAction as CFString) == .success else {
        fatalError("Menu path press failed: \(title)")
    }
    Thread.sleep(forTimeInterval: 0.2)
}
print("PASS native path \(CommandLine.arguments.dropFirst(2).joined(separator: " / "))")
