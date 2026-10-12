import Cocoa
import ApplicationServices

let pid = Int32(CommandLine.arguments[1])!
precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func buttons(_ element: AXUIElement) -> [AXUIElement] {
    var matches: [AXUIElement] = []
    if attribute(element, kAXRoleAttribute) as? String == kAXButtonRole,
       attribute(element, kAXTitleAttribute) as? String == "本文" {
        matches.append(element)
    }
    for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
        matches += buttons(child)
    }
    return matches
}
let matches = buttons(AXUIElementCreateApplication(pid))
precondition(matches.count == 1)
precondition(attribute(matches[0], kAXEnabledAttribute) as? Bool == false)
let result = AXUIElementPerformAction(matches[0], kAXPressAction as CFString)
print("AXPress=\(result.rawValue)")
precondition(result == .success)
