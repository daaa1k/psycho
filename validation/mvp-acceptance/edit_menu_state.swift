import Cocoa
import ApplicationServices

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func describe(_ element: AXUIElement, depth: Int = 0) -> [String: Any] {
    var result: [String: Any] = [:]
    for (name, key) in [(kAXRoleAttribute, "role"), (kAXTitleAttribute, "title"), (kAXEnabledAttribute, "enabled")] {
        if let value = attribute(element, name) { result[key] = value }
    }
    if depth < 8, let children = attribute(element, kAXChildrenAttribute) as? [AXUIElement] {
        result["children"] = children.map { describe($0, depth: depth + 1) }
    }
    return result
}
guard [2, 3].contains(CommandLine.arguments.count), let pid = Int32(CommandLine.arguments[1]), pid > 0 else {
    fatalError("Usage: edit-menu-state OWNED_PID")
}
let app = AXUIElementCreateApplication(pid)
var bar: CFTypeRef?
guard AXUIElementCopyAttributeValue(app, kAXMenuBarAttribute as CFString, &bar) == .success,
      let bar = bar, CFGetTypeID(bar) == AXUIElementGetTypeID() else {
    fatalError("Owned process has no readable AX menu bar")
}
let menuBar = unsafeBitCast(bar, to: AXUIElement.self)
let editMenus = attribute(menuBar, kAXChildrenAttribute) as? [AXUIElement] ?? []
if CommandLine.arguments.count == 3 {
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
        fatalError("Owned process must be foreground for native menu actions")
    }
    func matching(_ element: AXUIElement, title: String) -> [AXUIElement] {
        var result: [AXUIElement] = []
        if attribute(element, kAXTitleAttribute) as? String == title { result.append(element) }
        if let children = attribute(element, kAXChildrenAttribute) as? [AXUIElement] {
            result += children.flatMap { matching($0, title: title) }
        }
        return result
    }
    let matches = editMenus.flatMap { matching($0, title: CommandLine.arguments[2]) }
    guard matches.count == 1, attribute(matches[0], kAXEnabledAttribute) as? Bool == true,
          AXUIElementPerformAction(matches[0], kAXPressAction as CFString) == .success else {
        fatalError("Native menu action must uniquely match an enabled owned item")
    }
}
let result: [String: Any] = ["pid": pid, "menuBar": ["role": "AXMenuBar", "children": editMenus.map { describe($0) }]]
let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
print(String(data: data, encoding: .utf8)!)
