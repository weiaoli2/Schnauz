import AppKit
import Foundation

// A source-defined icon so future branding changes stay editable.
func drawIcon(_ size: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let scale = CGFloat(size) / 1024
    let transform = NSAffineTransform()
    transform.scale(by: scale)
    transform.concat()
    let frame = NSRect(x: 80, y: 80, width: 864, height: 864)
    let background = NSBezierPath(roundedRect: frame, xRadius: 194, yRadius: 194)
    NSGradient(starting: NSColor(srgbRed: 0.48, green: 0.48, blue: 0.99, alpha: 1), ending: NSColor(srgbRed: 0.25, green: 0.26, blue: 0.75, alpha: 1))!.draw(in: background, angle: -65)
    NSColor.white.withAlphaComponent(0.17).setFill()
    NSBezierPath(roundedRect: NSRect(x: 189, y: 240, width: 646, height: 565), xRadius: 62, yRadius: 62).fill()
    NSColor.white.withAlphaComponent(0.96).setFill()
    NSBezierPath(roundedRect: NSRect(x: 191, y: 220, width: 642, height: 536), xRadius: 58, yRadius: 58).fill()
    NSColor(srgbRed: 0.88, green: 0.88, blue: 0.98, alpha: 1).setFill()
    NSBezierPath(roundedRect: NSRect(x: 215, y: 244, width: 167, height: 488), xRadius: 38, yRadius: 38).fill()
    for (y, width) in [(622, 102), (553, 82), (484, 96)] {
        NSColor(srgbRed: 0.49, green: 0.49, blue: 0.84, alpha: 1).setFill()
        NSBezierPath(roundedRect: NSRect(x: 247, y: y, width: width, height: 16), xRadius: 8, yRadius: 8).fill()
    }
    for y in [585, 455, 325] {
        NSColor(srgbRed: 0.43, green: 0.44, blue: 0.90, alpha: 1).setFill()
        NSBezierPath(roundedRect: NSRect(x: 424, y: y, width: 66, height: 48), xRadius: 11, yRadius: 11).fill()
        NSColor(srgbRed: 0.78, green: 0.79, blue: 0.91, alpha: 1).setFill()
        NSBezierPath(roundedRect: NSRect(x: 517, y: y + 17, width: 234, height: 13), xRadius: 6, yRadius: 6).fill()
    }
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}
let destination = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    try drawIcon(size).write(to: destination.appendingPathComponent("icon_\(size)x\(size).png"))
    try drawIcon(size * 2).write(to: destination.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
try drawIcon(1024).write(to: destination.deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("icon_base.png"))
