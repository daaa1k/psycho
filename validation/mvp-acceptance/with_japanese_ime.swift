import Carbon
import Foundation

var childPID: pid_t = 0
var receivedSignal: Int32 = 0

func forwardSignal(_ value: Int32) {
    receivedSignal = value
    if childPID > 0 {
        kill(childPID, SIGTERM)
    }
}

signal(SIGINT, forwardSignal)
signal(SIGTERM, forwardSignal)

func property(_ source: TISInputSource, _ key: CFString) -> AnyObject {
    Unmanaged<AnyObject>.fromOpaque(TISGetInputSourceProperty(source, key)).takeUnretainedValue()
}

func identifier(_ source: TISInputSource) -> String {
    property(source, kTISPropertyInputSourceID) as! String
}

func check(_ status: OSStatus) throws {
    if status != noErr {
        throw NSError(domain: NSOSStatusErrorDomain, code: Int(status))
    }
}

func run() throws -> Int32 {
    let sources = TISCreateInputSourceList(nil, true).takeRetainedValue() as! [TISInputSource]
    let original = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
    let japanese = sources.first {
        identifier($0) == "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese"
    }!
    let parent = sources.first {
        identifier($0) == "com.apple.inputmethod.Kotoeri.RomajiTyping"
    }!
    let enabledBefore = sources.filter {
        identifier($0).hasPrefix("com.apple.inputmethod.Kotoeri.")
    }.map { (source: $0, enabled: property($0, kTISPropertyInputSourceIsEnabled) as! Bool) }
    var changed: [TISInputSource] = []
    defer {
        let restored = TISSelectInputSource(original)
        for source in changed.reversed() {
            let status = TISDisableInputSource(source)
            precondition(status == noErr, "Could not restore input source enablement")
        }
        precondition(restored == noErr, "Could not restore selected input source")
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.3))
        let current = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
        precondition(identifier(current) == identifier(original))
        for entry in enabledBefore {
            precondition((property(entry.source, kTISPropertyInputSourceIsEnabled) as! Bool) == entry.enabled,
                         "Input source enablement differs after restoration")
        }
        print("restored=\(identifier(current)); enablement_restored=true")
    }
    for source in [parent, japanese] where !(property(source, kTISPropertyInputSourceIsEnabled) as! Bool) {
        try check(TISEnableInputSource(source))
        changed.append(source)
    }
    try check(TISSelectInputSource(japanese))
    print("selected=\(identifier(japanese))")
    if receivedSignal != 0 { return 128 + receivedSignal }
    let process = Process()
    process.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
    process.arguments = Array(CommandLine.arguments.dropFirst(2))
    try process.run()
    childPID = process.processIdentifier
    if receivedSignal != 0 { process.terminate() }
    process.waitUntilExit()
    childPID = 0
    return receivedSignal == 0 ? process.terminationStatus : 128 + receivedSignal
}

do {
    exit(try run())
} catch {
    print(error)
    exit(1)
}
