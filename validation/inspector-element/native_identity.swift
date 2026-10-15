import AppKit
import ApplicationServices

let pid = Int32(CommandLine.arguments[1])!
precondition(NSRunningApplication(processIdentifier: pid)?.executableURL?.lastPathComponent == "psycho")
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func nodes(_ element: AXUIElement, path: [String]) -> [[String: Any]] {
    let title = attribute(element, kAXTitleAttribute) as? String ?? ""
    let description = attribute(element, kAXDescriptionAttribute) as? String ?? ""
    let label = title.isEmpty ? description : title
    var row: [String: Any] = ["label": label, "role": attribute(element, kAXRoleAttribute) as? String ?? "", "nativeIdentity": CFHash(element), "parents": path]
    for name in ["AXIdentifier", "AXSelected", "AXEnabled", "AXFocused"] {
        if let value = attribute(element, name) { row[name] = value }
    }
    if let value = attribute(element, kAXPositionAttribute), CFGetTypeID(value) == AXValueGetTypeID() {
        var point = CGPoint.zero
        AXValueGetValue(value as! AXValue, .cgPoint, &point)
        row["position"] = [point.x, point.y]
    }
    if let value = attribute(element, kAXSizeAttribute), CFGetTypeID(value) == AXValueGetTypeID() {
        var size = CGSize.zero
        AXValueGetValue(value as! AXValue, .cgSize, &size)
        row["size"] = [size.width, size.height]
    }
    let children = attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
    return [row] + children.flatMap { nodes($0, path: path + [label]) }
}
let result = nodes(AXUIElementCreateApplication(pid), path: [])
print(String(data: try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]), encoding: .utf8)!)
