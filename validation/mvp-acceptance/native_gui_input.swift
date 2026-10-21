import AppKit
import ApplicationServices
import CoreGraphics

struct InputError: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}

struct Chord {
    let name: String
    let key: CGKeyCode
    let flags: CGEventFlags

    init(_ name: String) throws {
        let parts = name.split(separator: "+", omittingEmptySubsequences: false).map { $0.lowercased() }
        let keys: [String: CGKeyCode] = [
            "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7,
            "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14, "r": 15,
            "y": 16, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21, "6": 22,
            "5": 23, "9": 25, "7": 26, "8": 28, "0": 29, "o": 31, "u": 32,
            "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46,
            "return": 36, "enter": 36, "escape": 53, "tab": 48, "backspace": 51,
            "delete": 117, "space": 49, "left": 123, "right": 124, "down": 125,
            "up": 126, "home": 115, "end": 119, "pageup": 116, "pagedown": 121,
        ]
        guard let last = parts.last, let key = keys[last] else {
            throw InputError("Unsupported hardware key in chord '\(name)'")
        }
        var flags: CGEventFlags = []
        for modifier in parts.dropLast() {
            switch modifier {
            case "cmdorctrl", "cmd", "command": flags.insert(.maskCommand)
            case "ctrl", "control": flags.insert(.maskControl)
            case "alt", "option": flags.insert(.maskAlternate)
            case "shift": flags.insert(.maskShift)
            default: throw InputError("Unsupported modifier '\(modifier)' in chord '\(name)'")
            }
        }
        self.name = name
        self.key = key
        self.flags = flags
    }
}

enum Operation {
    case focusInput(String)
    case key(Chord)
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}

func frame(_ element: AXUIElement) -> CGRect? {
    guard let position = attribute(element, kAXPositionAttribute),
          let size = attribute(element, kAXSizeAttribute),
          CFGetTypeID(position) == AXValueGetTypeID(),
          CFGetTypeID(size) == AXValueGetTypeID() else { return nil }
    var point = CGPoint.zero
    var dimensions = CGSize.zero
    guard AXValueGetValue(position as! AXValue, .cgPoint, &point),
          AXValueGetValue(size as! AXValue, .cgSize, &dimensions),
          point.x.isFinite, point.y.isFinite,
          dimensions.width.isFinite, dimensions.height.isFinite,
          dimensions.width > 0, dimensions.height > 0 else { return nil }
    return CGRect(origin: point, size: dimensions)
}

func validateTarget(_ pid: pid_t, executable: URL) throws {
    guard let app = NSRunningApplication(processIdentifier: pid), !app.isTerminated,
          app.executableURL?.resolvingSymlinksInPath().standardizedFileURL == executable else {
        throw InputError("PID \(pid) does not run expected executable '\(executable.path)'")
    }
    guard !app.isHidden, NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
        throw InputError("PID \(pid) is not the visible frontmost application")
    }
    guard AXIsProcessTrusted() else { throw InputError("Accessibility permission is required") }
}

func focusInput(_ label: String, pid: pid_t, executable: URL) throws {
    let app = AXUIElementCreateApplication(pid)
    var displays = [CGDirectDisplayID](repeating: 0, count: 32)
    var displayCount: UInt32 = 0
    guard CGGetActiveDisplayList(UInt32(displays.count), &displays, &displayCount) == .success else {
        throw InputError("Cannot read active displays")
    }
    let screenBounds = displays.prefix(Int(displayCount)).map { CGDisplayBounds($0) }
    var matches: [(field: AXUIElement, point: CGPoint)] = []
    var visited = Set<AXUIElement>()
    func visit(_ element: AXUIElement, window: AXUIElement?) {
        guard visited.insert(element).inserted,
              attribute(element, "AXHidden") as? Bool != true else { return }
        let role = attribute(element, kAXRoleAttribute) as? String
        let containingWindow = role == kAXWindowRole ? element : window
        if role == kAXTextFieldRole,
           attribute(element, kAXEnabledAttribute) as? Bool == true {
            let title = attribute(element, kAXTitleAttribute) as? String ?? ""
            let description = attribute(element, kAXDescriptionAttribute) as? String ?? ""
            if (title.isEmpty ? description : title) == label,
               let window = containingWindow,
               attribute(window, kAXMinimizedAttribute) as? Bool != true,
               let fieldBounds = frame(element), let windowBounds = frame(window) {
                let point = CGPoint(x: fieldBounds.midX, y: fieldBounds.midY)
                if windowBounds.contains(point), screenBounds.contains(where: { $0.contains(point) }) {
                    matches.append((element, point))
                }
            }
        }
        for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
            visit(child, window: containingWindow)
        }
    }
    visit(app, window: nil)
    guard matches.count == 1, let target = matches.first else {
        throw InputError("Expected one enabled visible AXTextField '\(label)' for PID \(pid), found \(matches.count)")
    }
    let events = try [CGEventType.leftMouseDown, .leftMouseUp].map { kind in
        guard let event = CGEvent(mouseEventSource: nil, mouseType: kind,
                                  mouseCursorPosition: target.point, mouseButton: .left) else {
            throw InputError("Cannot create mouse event for '\(label)'")
        }
        event.flags = []
        event.setIntegerValueField(.mouseEventClickState, value: 1)
        return event
    }
    try validateTarget(pid, executable: executable)
    for event in events {
        event.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.04)
    }
    let deadline = ProcessInfo.processInfo.systemUptime + 2
    repeat {
        if attribute(target.field, kAXFocusedAttribute) as? Bool == true { return }
        if let focused = attribute(app, kAXFocusedUIElementAttribute), CFEqual(focused, target.field) { return }
        Thread.sleep(forTimeInterval: 0.02)
    } while ProcessInfo.processInfo.systemUptime < deadline
    throw InputError("Dispatched click for '\(label)' to PID \(pid), but exact field AX focus was not confirmed; input was not resent")
}

func run() throws {
    let args = CommandLine.arguments
    guard args.count == 5, let pid = pid_t(args[1]), pid > 0,
          (args[2] as NSString).isAbsolutePath else {
        throw InputError("Usage: native-gui-input PID EXPECTED_EXECUTABLE focus-input EXACT_LABEL | key CHORD")
    }
    let operation: Operation
    switch args[3] {
    case "focus-input":
        guard !args[4].isEmpty else { throw InputError("Input label must not be empty") }
        operation = .focusInput(args[4])
    case "key": operation = .key(try Chord(args[4]))
    default: throw InputError("Unsupported operation '\(args[3])'")
    }
    let executable = URL(fileURLWithPath: args[2]).resolvingSymlinksInPath().standardizedFileURL
    try validateTarget(pid, executable: executable)
    var receipt: [String: Any] = ["pid": pid, "dispatched": true]
    switch operation {
    case .focusInput(let label):
        try focusInput(label, pid: pid, executable: executable)
        receipt["label"] = label
        receipt["focused"] = true
    case .key(let chord):
        let modifiers: [(CGKeyCode, CGEventFlags)] = [
            (55, .maskCommand), (59, .maskControl), (58, .maskAlternate), (56, .maskShift),
        ].filter { chord.flags.contains($0.1) }
        var events: [CGEvent] = []
        var held: CGEventFlags = []
        for (key, flag) in modifiers {
            held.insert(flag)
            guard let event = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: true) else {
                throw InputError("Cannot create modifier event for '\(chord.name)'")
            }
            event.type = .flagsChanged
            event.flags = held
            events.append(event)
        }
        events += try [true, false].map { down in
            guard let event = CGEvent(keyboardEventSource: nil, virtualKey: chord.key, keyDown: down) else {
                throw InputError("Cannot create keyboard event for '\(chord.name)'")
            }
            event.flags = chord.flags
            return event
        }
        for (key, flag) in modifiers.reversed() {
            held.remove(flag)
            guard let event = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: false) else {
                throw InputError("Cannot create modifier release for '\(chord.name)'")
            }
            event.type = .flagsChanged
            event.flags = held
            events.append(event)
        }
        try validateTarget(pid, executable: executable)
        for event in events {
            event.post(tap: .cghidEventTap)
            Thread.sleep(forTimeInterval: 0.04)
        }
        receipt["chord"] = chord.name
    }
    let data = try JSONSerialization.data(withJSONObject: receipt, options: [.sortedKeys])
    print(String(decoding: data, as: UTF8.self))
}

do {
    try run()
} catch {
    FileHandle.standardError.write(Data("native-gui-input: \(error)\n".utf8))
    exit(1)
}
