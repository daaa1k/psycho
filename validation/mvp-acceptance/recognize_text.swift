import Foundation
import Vision
import AppKit
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["ja-JP", "en-US"]
try VNImageRequestHandler(url: url).perform([request])
for observation in request.results ?? [] {
    guard let candidate = observation.topCandidates(1).first else { continue }
    print(candidate.string)
}
