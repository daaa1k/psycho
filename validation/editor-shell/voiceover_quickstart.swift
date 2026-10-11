import Cocoa
import ApplicationServices
let pid = pid_t(CommandLine.arguments[1])!
precondition(NSRunningApplication(processIdentifier: pid)?.bundleIdentifier == "com.apple.VoiceOverQuickstart")
let app = AXUIElementCreateApplication(pid)
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func find(_ element: AXUIElement) -> [AXUIElement] {
    var matches: [AXUIElement] = []
    if attribute(element, kAXTitleAttribute) as? String == CommandLine.arguments[2] { matches.append(element) }
    for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] { matches += find(child) }
    return matches
}
let matches = find(app)
precondition(matches.count == 1)
print(AXUIElementPerformAction(matches[0], kAXPressAction as CFString).rawValue)
