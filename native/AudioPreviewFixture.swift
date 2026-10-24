// Silent, disposable PCM and AAC fixtures. No user media or audio devices.
import AVFoundation
import Foundation

let directory = URL(fileURLWithPath: CommandLine.arguments[1])
let format = AVAudioFormat(standardFormatWithSampleRate: 44100, channels: 1)!
let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 88200)!
buffer.frameLength = buffer.frameCapacity
memset(buffer.floatChannelData![0], 0, Int(buffer.frameLength) * MemoryLayout<Float>.size)
for (name, settings) in [
    ("sample.wav", format.settings),
    ("sample.m4a", [AVFormatIDKey: kAudioFormatMPEG4AAC,
                    AVSampleRateKey: 44100, AVNumberOfChannelsKey: 1,
                    AVEncoderBitRateKey: 64000] as [String: Any])
] {
    let file = try AVAudioFile(forWriting: directory.appendingPathComponent(name), settings: settings)
    try file.write(from: buffer)
}
