// Sign-in QR codes (`AuthStep::ShowQr`), drawn with Core Image's generator: crisp modules, a
// quiet zone, dark on light whatever the appearance so phone cameras read it.

import AppKit
import CoreImage
import CoreImage.CIFilterBuiltins

enum QRCode {
    /// The code for `text` with each module `scale` pixels wide, or nil when it does not fit.
    static func image(for text: String, scale: CGFloat = 8) -> NSImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        let context = CIContext(options: [.useSoftwareRenderer: false])
        guard let cgImage = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return NSImage(cgImage: cgImage, size: NSSize(width: scaled.extent.width / 2, height: scaled.extent.height / 2))
    }
}
