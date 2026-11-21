// Generate the repository's GitHub social preview using the rmux mark.
import AppKit
import Foundation

let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 1280,
    pixelsHigh: 640, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
    isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
NSColor(srgbRed: 21/255, green: 21/255, blue: 23/255, alpha: 1).setFill()
NSBezierPath(rect: NSRect(x: 0, y: 0, width: 1280, height: 640)).fill()
let mark = NSImage(contentsOfFile: "docs/brand/github-avatar.png")!
mark.draw(in: NSRect(x: 88, y: 298, width: 180, height: 180))
func label(_ text: String, x: CGFloat, y: CGFloat, size: CGFloat,
           color: NSColor, weight: NSFont.Weight = .regular) {
    (text as NSString).draw(at: NSPoint(x: x, y: y), withAttributes: [
        .font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color,
    ])
}
label("rmux", x: 300, y: 315, size: 116, color: .white, weight: .semibold)
label("A terminal workspace built for responsiveness.", x: 96, y: 220,
      size: 37, color: NSColor(white: 0.84, alpha: 1))
label("Rust  ·  GPU-rendered  ·  Ghostty", x: 96, y: 150,
      size: 27, color: NSColor(srgbRed: 185/255, green: 175/255, blue: 1, alpha: 1))
label("rmux.app", x: 96, y: 64, size: 23, color: NSColor(white: 0.62, alpha: 1))
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!.write(
    to: URL(fileURLWithPath: "docs/brand/github-social.png"))
