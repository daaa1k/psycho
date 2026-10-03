// Use window-local coordinates and restrict synthetic input to our scratch app.
import AppKit
import CoreGraphics
import ApplicationServices
let a=CommandLine.arguments
precondition(a.count == 6 || a.count == 7, "usage: slow_drag PID x1 y1 x2 y2 [hold_seconds]")
let pid=pid_t(a[1])!
precondition(NSRunningApplication(processIdentifier:pid)?.localizedName == "psycho")
precondition(AXIsProcessTrusted(), "Accessibility permission is required")
let app=AXUIElementCreateApplication(pid)
var value: CFTypeRef?
precondition(AXUIElementCopyAttributeValue(app,kAXWindowsAttribute as CFString,&value) == .success)
let window=(value as! [AXUIElement])[0]
precondition(AXUIElementCopyAttributeValue(window,kAXPositionAttribute as CFString,&value) == .success)
var origin=CGPoint.zero
precondition(AXValueGetValue(value as! AXValue,.cgPoint,&origin))
precondition(NSRunningApplication(processIdentifier:pid)?.isActive == true, "psycho must be focused")
let start=CGPoint(x:origin.x+Double(a[2])!,y:origin.y+Double(a[3])!)
let end=CGPoint(x:origin.x+Double(a[4])!,y:origin.y+Double(a[5])!)
let hold=a.count == 7 ? Double(a[6])! : 1
precondition(hold >= 0 && hold <= 30)
func post(_ type: CGEventType, _ point: CGPoint) {
    let event=CGEvent(mouseEventSource:nil,mouseType:type,mouseCursorPosition:point,mouseButton:.left)!
    event.setIntegerValueField(.mouseEventClickState,value:1)
    event.post(tap:.cghidEventTap)
}
post(.mouseMoved,start);Thread.sleep(forTimeInterval:0.2)
post(.leftMouseDown,start);Thread.sleep(forTimeInterval:0.3)
for step in 1...20 {
    let t=CGFloat(step)/20
    post(.leftMouseDragged,CGPoint(x:start.x+(end.x-start.x)*t,y:start.y+(end.y-start.y)*t))
    Thread.sleep(forTimeInterval:0.08)
}
Thread.sleep(forTimeInterval:hold)
post(.leftMouseUp,end)
