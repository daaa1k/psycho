import ApplicationServices
import Foundation
import AppKit
let pid=pid_t(CommandLine.arguments[1])!
precondition(NSRunningApplication(processIdentifier:pid)?.localizedName == "psycho")
let app=AXUIElementCreateApplication(pid)
var value: CFTypeRef?
precondition(AXUIElementCopyAttributeValue(app,kAXWindowsAttribute as CFString,&value) == .success)
let window=(value as! [AXUIElement])[0]
var size=CGSize(width:Double(CommandLine.arguments[2])!,height:Double(CommandLine.arguments[3])!)
let result=AXUIElementSetAttributeValue(window,kAXSizeAttribute as CFString,AXValueCreate(.cgSize,&size)!)
precondition(result == .success, "AX window resize failed")
