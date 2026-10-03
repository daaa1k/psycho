import Foundation
import Vision
import AppKit
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let image = NSImage(contentsOf: url)!
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["ja-JP", "en-US"]
try VNImageRequestHandler(url: url).perform([request])
let rows: [[String: Any]] = (request.results ?? []).compactMap { observation in
    guard let candidate = observation.topCandidates(1).first else { return nil }
    let b = observation.boundingBox
    return ["text": candidate.string, "x": b.midX * image.size.width,
            "y": (1 - b.midY) * image.size.height,
            "width": b.width * image.size.width, "height": b.height * image.size.height]
}
let data = try JSONSerialization.data(withJSONObject: rows, options: .sortedKeys)
print(String(data: data, encoding: .utf8)!)
