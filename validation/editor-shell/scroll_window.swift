import AppKit
import ScreenCaptureKit
@main struct Main {
 @MainActor static func main() async throws {
  _ = NSApplication.shared
  let a=CommandLine.arguments; let pid=Int32(a[1])!
  let app=NSRunningApplication(processIdentifier:pid)!
  precondition(app.executableURL?.lastPathComponent == "psycho")
  if a[2] == "activate" { app.activate(); return }
  precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
  let content=try await SCShareableContent.excludingDesktopWindows(true,onScreenWindowsOnly:true)
  let w=content.windows.first { $0.owningApplication?.processID == pid && $0.windowLayer == 0 && $0.frame.width > 500 }!
  if a[2] == "capture" {
   let c=SCStreamConfiguration(); c.width=Int(w.frame.width)*2;c.height=Int(w.frame.height)*2;c.scalesToFit=true;c.showsCursor=false;c.ignoreShadowsSingleWindow=true
   let image=try await SCScreenshotManager.captureImage(contentFilter:SCContentFilter(desktopIndependentWindow:w),configuration:c)
   try NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:])!.write(to:URL(fileURLWithPath:a[3]))
  } else {
   let p=CGPoint(x:w.frame.minX+600,y:w.frame.minY+400)
   CGEvent(mouseEventSource:nil,mouseType:.mouseMoved,mouseCursorPosition:p,mouseButton:.left)!.post(tap:.cghidEventTap)
   try await Task.sleep(for:.milliseconds(300))
   precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid,
                "Scroll target lost foreground to \(NSWorkspace.shared.frontmostApplication?.localizedName ?? "unknown") pid \(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1)")
   for type in [CGEventType.leftMouseDown, .leftMouseUp] { CGEvent(mouseEventSource:nil,mouseType:type,mouseCursorPosition:p,mouseButton:.left)!.post(tap:.cghidEventTap) }
   try await Task.sleep(for:.milliseconds(300))
   precondition(NSWorkspace.shared.frontmostApplication?.processIdentifier == pid)
   let e=CGEvent(scrollWheelEvent2Source:nil,units:.pixel,wheelCount:2,wheel1:Int32(a[3])!,wheel2:Int32(a[4])!,wheel3:0)!
   e.location=p; e.post(tap:.cghidEventTap)
   try await Task.sleep(for:.milliseconds(300))
  }
 }
}
