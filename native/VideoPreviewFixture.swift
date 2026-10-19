// Synthetic local H.264 fixture. No user media, accounts or app windows.
import AVFoundation
import CoreVideo
import Foundation

let output = URL(fileURLWithPath: CommandLine.arguments[1])
let writer = try AVAssetWriter(outputURL: output, fileType: .mp4)
let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
    AVVideoCodecKey: AVVideoCodecType.h264,
    AVVideoWidthKey: 320, AVVideoHeightKey: 180
])
let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input,
    sourcePixelBufferAttributes: [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32ARGB,
        kCVPixelBufferWidthKey as String: 320, kCVPixelBufferHeightKey as String: 180])
writer.add(input)
precondition(writer.startWriting())
writer.startSession(atSourceTime: .zero)
for frame in 0..<30 {
    let deadline = Date().addingTimeInterval(5)
    while !input.isReadyForMoreMediaData && Date() < deadline { Thread.sleep(forTimeInterval: 0.002) }
    precondition(input.isReadyForMoreMediaData)
    var optional: CVPixelBuffer?
    precondition(CVPixelBufferCreate(kCFAllocatorDefault,320,180,kCVPixelFormatType_32ARGB,nil,&optional)==kCVReturnSuccess)
    let buffer = optional!
    CVPixelBufferLockBaseAddress(buffer,[])
    memset(CVPixelBufferGetBaseAddress(buffer)!, frame % 2 == 0 ? 100 : 160, CVPixelBufferGetBytesPerRow(buffer)*180)
    CVPixelBufferUnlockBaseAddress(buffer,[])
    precondition(adaptor.append(buffer,withPresentationTime:CMTime(value:Int64(frame),timescale:30)))
}
input.markAsFinished()
let done = DispatchSemaphore(value:0)
writer.finishWriting { done.signal() }
precondition(done.wait(timeout:.now()+10) == .success)
precondition(writer.status == .completed)
