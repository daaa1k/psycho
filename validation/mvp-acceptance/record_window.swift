import AppKit
import AVFoundation
import ScreenCaptureKit

private struct RecordingError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

private final class RecordingStatus: NSObject, SCRecordingOutputDelegate, SCStreamDelegate {
    enum State { case waiting, recording, finished, failed(Error) }
    private let lock = NSLock()
    private var value: State = .waiting
    private var requestedStop = false
    var state: State { lock.withLock { value } }
    var stopRequested: Bool { lock.withLock { requestedStop } }
    func requestStop() { lock.withLock { requestedStop = true } }
    private func set(_ state: State) {
        lock.withLock {
            if case .failed = value { return }
            value = state
        }
    }
    func recordingOutputDidStartRecording(_ recordingOutput: SCRecordingOutput) { set(.recording) }
    func recordingOutputDidFinishRecording(_ recordingOutput: SCRecordingOutput) { set(.finished) }
    func recordingOutput(_ recordingOutput: SCRecordingOutput, didFailWithError error: Error) { set(.failed(error)) }
    func stream(_ stream: SCStream, didStopWithError error: Error) { set(.failed(error)) }
}

@main
struct RecordWindow {
    static func main() async {
        do { try await record() }
        catch {
            FileHandle.standardError.write(Data("record_window: \(error)\n".utf8))
            exit(1)
        }
    }

    @MainActor static func record() async throws {
        _ = NSApplication.shared
        let args = CommandLine.arguments
        guard args.count == 4, let pid = Int32(args[1]), pid > 0,
              let seconds = Double(args[3]), seconds.isFinite, seconds > 0, seconds <= 3600 else {
            throw RecordingError("usage: record_window PID OUTPUT.mov SECONDS (0 < seconds <= 3600)")
        }
        guard let app = NSRunningApplication(processIdentifier: pid),
              app.executableURL?.lastPathComponent == "psycho", !app.isTerminated else {
            throw RecordingError("PID must belong to a running psycho executable")
        }
        let output = URL(fileURLWithPath: args[2]).standardizedFileURL
        guard output.pathExtension.lowercased() == "mov" else { throw RecordingError("output must end in .mov") }
        guard !FileManager.default.fileExists(atPath: output.path) else { throw RecordingError("refusing existing output") }
        guard CGPreflightScreenCaptureAccess() else { throw RecordingError("screen recording permission is required") }
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        let windows = content.windows.filter { $0.owningApplication?.processID == pid && $0.windowLayer == 0 && $0.frame.width > 100 && $0.frame.height > 100 }
        guard windows.count == 1, let window = windows.first else {
            throw RecordingError("expected one visible psycho main window, found \(windows.count)")
        }
        let configuration = SCStreamConfiguration()
        configuration.width = 1920
        configuration.height = 1080
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 30)
        configuration.scalesToFit = true
        configuration.showsCursor = true
        configuration.capturesAudio = false
        configuration.captureMicrophone = false
        configuration.ignoreShadowsSingleWindow = true
        let status = RecordingStatus()
        let stopSignals = [SIGTERM, SIGINT].map { number in
            signal(number, SIG_IGN)
            let source = DispatchSource.makeSignalSource(signal: number, queue: .global())
            source.setEventHandler { status.requestStop() }
            source.resume()
            return source
        }
        defer { stopSignals.forEach { $0.cancel() } }
        let stream = SCStream(filter: SCContentFilter(desktopIndependentWindow: window), configuration: configuration, delegate: status)
        let recordingConfiguration = SCRecordingOutputConfiguration()
        recordingConfiguration.outputURL = output
        recordingConfiguration.outputFileType = .mov
        recordingConfiguration.videoCodecType = .h264
        let recording = SCRecordingOutput(configuration: recordingConfiguration, delegate: status)
        try stream.addRecordingOutput(recording)
        try await stream.startCapture()
        do {
            let startDeadline = Date().addingTimeInterval(15)
            while true {
                switch status.state {
                case .recording: break
                case .failed(let error): throw error
                case .finished: throw RecordingError("recording ended before capture started")
                case .waiting:
                    guard Date() < startDeadline else { throw RecordingError("recording start timed out") }
                    try await Task.sleep(for: .milliseconds(100))
                    continue
                }
                break
            }
            print("RECORDING pid=\(pid) window=\(window.windowID) output=\(output.path)")
            fflush(stdout)
            let deadline = Date().addingTimeInterval(seconds)
            while Date() < deadline && !status.stopRequested {
                try await Task.sleep(for: .milliseconds(250))
                if status.stopRequested { break }
                if case .failed(let error) = status.state { throw error }
                if case .finished = status.state { throw RecordingError("recording ended unexpectedly") }
                guard !app.isTerminated else { throw RecordingError("psycho exited during recording") }
                let current = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: false)
                guard current.windows.contains(where: { $0.windowID == window.windowID && $0.owningApplication?.processID == pid }) else {
                    throw RecordingError("target window disappeared or changed identity; recording is incomplete")
                }
            }
            try await stream.stopCapture()
            let finishDeadline = Date().addingTimeInterval(15)
            while true {
                switch status.state {
                case .finished: break
                case .failed(let error): throw error
                default:
                    guard Date() < finishDeadline else { throw RecordingError("recording finalization timed out") }
                    try await Task.sleep(for: .milliseconds(100))
                    continue
                }
                break
            }
        } catch {
            try? await stream.stopCapture()
            throw error
        }
        let asset = AVURLAsset(url: output)
        let duration = try await asset.load(.duration).seconds
        let tracks = try await asset.loadTracks(withMediaType: .video)
        guard duration > 0, !tracks.isEmpty else { throw RecordingError("output contains no playable video") }
        print("FINISHED duration=\(duration) seconds output=\(output.path)")
    }
}
