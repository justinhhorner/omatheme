import AppKit
import OmarchyThemesKit

extension NSColor {
    convenience init(_ color: RgbColor) {
        let c = color.unitComponents
        self.init(srgbRed: c.red, green: c.green, blue: c.blue, alpha: 1)
    }
}

extension RgbColor {
    /// The color in sRGB, or nil if it has no sRGB equivalent (e.g. a pattern color).
    init?(_ color: NSColor) {
        guard let srgb = color.usingColorSpace(.sRGB) else { return nil }
        func byte(_ v: CGFloat) -> UInt8 { UInt8((min(max(v, 0), 1) * 255).rounded()) }
        self.init(r: byte(srgb.redComponent), g: byte(srgb.greenComponent), b: byte(srgb.blueComponent))
    }
}
