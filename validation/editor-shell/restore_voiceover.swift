import Cocoa
import ApplicationServices

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func find(_ element: AXUIElement, title: String) -> [AXUIElement] {
    var matches: [AXUIElement] = []
    if attribute(element, kAXTitleAttribute) as? String == title { matches.append(element) }
    for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
        matches += find(child, title: title)
    }
    return matches
}
let applications = NSWorkspace.shared.runningApplications
var record: [String: Any] = [:]
if let utility = applications.first(where: { $0.bundleIdentifier == "com.apple.VoiceOverUtility" }) {
    let app = AXUIElementCreateApplication(utility.processIdentifier)
    let matches = find(app, title: "AppleScriptによるVoiceOverの制御を許可")
    precondition(matches.count == 1)
    let initial = (attribute(matches[0], kAXValueAttribute) as! NSNumber).boolValue
    record["scriptingBeforeRestore"] = initial
    if initial {
        record["restoreScriptingAXResult"] = AXUIElementPerformAction(matches[0], kAXPressAction as CFString).rawValue
        Thread.sleep(forTimeInterval: 0.3)
    }
    let final = (attribute(matches[0], kAXValueAttribute) as! NSNumber).boolValue
    record["scriptingRestored"] = !final
    precondition(!final)
}
if let welcome = applications.first(where: { $0.bundleIdentifier == "com.apple.VoiceOverQuickstart" }) {
    let matches = find(AXUIElementCreateApplication(welcome.processIdentifier), title: "VoiceOverをオフにする")
    precondition(matches.count == 1)
    record["disableVoiceOverAXResult"] = AXUIElementPerformAction(matches[0], kAXPressAction as CFString).rawValue
    Thread.sleep(forTimeInterval: 0.5)
}
record["voiceOverEnabled"] = NSWorkspace.shared.isVoiceOverEnabled
let jsonData = try JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(jsonData)
