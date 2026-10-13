import Cocoa
import ApplicationServices

let names = ["scripting": "AppleScriptによるVoiceOverの制御を許可", "welcome": "VoiceOverの起動時にようこそダイアログを表示"]
guard let utility = NSWorkspace.shared.runningApplications.first(where: { $0.bundleIdentifier == "com.apple.VoiceOverUtility" }) else {
    fatalError("VoiceOver Utility must be open")
}
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func find(_ element: AXUIElement, _ title: String) -> [AXUIElement] {
    var matches = attribute(element, kAXTitleAttribute) as? String == title ? [element] : []
    for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
        matches += find(child, title)
    }
    return matches
}
let app = AXUIElementCreateApplication(utility.processIdentifier)
var controls: [String: AXUIElement] = [:]
for (key, title) in names {
    let matches = find(app, title)
    precondition(matches.count == 1, "General panel must expose \(title)")
    controls[key] = matches[0]
}
if CommandLine.arguments.count == 3 {
    let key = CommandLine.arguments[1]
    guard let control = controls[key] else { fatalError("Unknown checkbox") }
    let desired = CommandLine.arguments[2] == "1"
    utility.activate(options: [.activateAllWindows])
    Thread.sleep(forTimeInterval: 0.3)
    precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == utility.processIdentifier)
    let current = (attribute(control, kAXValueAttribute) as! NSNumber).boolValue
    if current != desired {
        precondition(AXUIElementPerformAction(control, kAXPressAction as CFString) == .success)
        Thread.sleep(forTimeInterval: 0.3)
    }
    precondition((attribute(control, kAXValueAttribute) as! NSNumber).boolValue == desired)
}
var result: [String: Any] = ["voiceOverEnabled": NSWorkspace.shared.isVoiceOverEnabled, "utilityPID": utility.processIdentifier]
for (key, control) in controls {
    result[key] = (attribute(control, kAXValueAttribute) as! NSNumber).boolValue
}
let data = try JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
