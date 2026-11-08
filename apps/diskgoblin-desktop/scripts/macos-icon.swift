import Cocoa
let output = URL(fileURLWithPath: CommandLine.arguments[1]).appendingPathComponent("AppIcon.iconset")
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = size * scale
        let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
        let context = NSGraphicsContext.current!.cgContext
        context.scaleBy(x: CGFloat(pixels) / 64, y: CGFloat(pixels) / 64)
        context.setFillColor(NSColor(srgbRed: 24/255, green: 28/255, blue: 27/255, alpha: 1).cgColor)
        context.fill(CGRect(x: 0, y: 0, width: 64, height: 64))
        context.setFillColor(NSColor(srgbRed: 196/255, green: 232/255, blue: 120/255, alpha: 1).cgColor)
        context.beginPath()
        context.move(to: CGPoint(x: 17, y: 40))
        for point in [(3, 49), (11, 27), (19, 25), (24, 15), (32, 12), (42, 16), (46, 25), (54, 28), (61, 49), (46, 40)] { context.addLine(to: CGPoint(x: point.0, y: point.1)) }
        context.addQuadCurve(to: CGPoint(x: 17, y: 40), control: CGPoint(x: 32, y: 54))
        context.closePath(); context.fillPath()
        context.setFillColor(NSColor(srgbRed: 24/255, green: 28/255, blue: 27/255, alpha: 1).cgColor)
        for points in [[(21, 35), (29, 32), (24, 28)], [(43, 35), (35, 32), (40, 28)], [(25, 23), (32, 20), (39, 23), (36, 17), (28, 17)]] {
            context.beginPath(); context.move(to: CGPoint(x: points[0].0, y: points[0].1))
            for p in points.dropFirst() { context.addLine(to: CGPoint(x: p.0, y: p.1)) }
            context.closePath(); context.fillPath()
        }
        NSGraphicsContext.restoreGraphicsState()
        let suffix = scale == 2 ? "@2x" : ""
        try bitmap.representation(using: .png, properties: [:])!.write(to: output.appendingPathComponent("icon_\(size)x\(size)\(suffix).png"))
    }
}
