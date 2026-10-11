import AppKit

guard CommandLine.arguments.count == 2,
      let data = try? Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])),
      let bitmap = NSBitmapImageRep(data: data) else {
    fatalError("Pass a screenshot PNG path")
}
var counts: [String: Int] = [:]
var channels = [Int](repeating: 0, count: bitmap.samplesPerPixel)
for y in 0..<bitmap.pixelsHigh {
    for x in 0..<bitmap.pixelsWide {
        bitmap.getPixel(&channels, atX: x, y: y)
        let key = String(format: "%02x%02x%02x", channels[0], channels[1], channels[2])
        counts[key, default: 0] += 1
    }
}
let jsonData = try JSONSerialization.data(withJSONObject: counts, options: [.sortedKeys])
FileHandle.standardOutput.write(jsonData)
