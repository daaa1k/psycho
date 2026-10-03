import Cocoa
let pid=Int(CommandLine.arguments[1])!
precondition(NSRunningApplication(processIdentifier:pid_t(pid))?.localizedName == "psycho")
let windows=CGWindowListCopyWindowInfo(.optionOnScreenOnly,kCGNullWindowID) as! [[String:Any]]
let own=windows.filter { $0[kCGWindowOwnerPID as String] as? Int == pid }.map { window in ["id":window[kCGWindowNumber as String]!,"bounds":window[kCGWindowBounds as String]!,"layer":window[kCGWindowLayer as String]!] }
let data=try! JSONSerialization.data(withJSONObject:own,options:.sortedKeys)
print(String(data:data,encoding:.utf8)!)
