import ApplicationServices
import Cocoa
let pid=pid_t(CommandLine.arguments[1])!
precondition(NSRunningApplication(processIdentifier:pid)?.localizedName=="psycho")
let app=AXUIElementCreateApplication(pid)
var value:CFTypeRef?
precondition(AXUIElementCopyAttributeValue(app,kAXWindowsAttribute as CFString,&value) == .success)
let windows=value as! [AXUIElement]
let window=windows.first { window in
 var value:CFTypeRef?
 guard AXUIElementCopyAttributeValue(window,kAXSizeAttribute as CFString,&value) == .success else { return false }
 var dimensions=CGSize.zero
 AXValueGetValue(value as! AXValue,.cgSize,&dimensions)
 return dimensions.width>1000
}!
var point=CGPoint(x:Double(CommandLine.arguments[2])!,y:Double(CommandLine.arguments[3])!)
precondition(AXUIElementSetAttributeValue(window,kAXPositionAttribute as CFString,AXValueCreate(.cgPoint,&point)!) == .success)
