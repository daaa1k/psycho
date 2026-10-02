import Foundation
import Vision
import AppKit
let image = NSImage(contentsOfFile: CommandLine.arguments[1])!
var rect = CGRect(origin: .zero, size: image.size)
let cg = image.cgImage(forProposedRect: &rect, context: nil, hints: nil)!
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["ja-JP", "en-US"]
request.usesLanguageCorrection = false
request.regionOfInterest = CommandLine.arguments.count > 2 && CommandLine.arguments[2] == "fullscreen"
    ? CGRect(x: 0, y: 0.8, width: 1, height: 0.2)
    : CGRect(x: 0, y: 0.64, width: 1, height: 0.15)
try VNImageRequestHandler(cgImage: cg).perform([request])
for result in request.results ?? [] { if let candidate = result.topCandidates(1).first { print(candidate.string) } }
