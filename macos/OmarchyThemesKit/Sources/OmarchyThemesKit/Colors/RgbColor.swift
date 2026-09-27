import Foundation

/// An opaque sRGB color. Encodes to JSON as "#rrggbb".
public struct RgbColor: Hashable, Sendable, CustomStringConvertible {
    public let r: UInt8
    public let g: UInt8
    public let b: UInt8

    public init(r: UInt8, g: UInt8, b: UInt8) {
        self.r = r
        self.g = g
        self.b = b
    }

    /// Accepts "#rrggbb", "rrggbb", "0xrrggbb", "#rgb" and "#rrggbbaa" (alpha ignored).
    public init?(hex text: String) {
        var s = Substring(text.trimmingCharacters(in: .whitespacesAndNewlines))
        if s.hasPrefix("#") {
            s = s.dropFirst()
        } else if s.lowercased().hasPrefix("0x") {
            s = s.dropFirst(2)
        }
        guard s.allSatisfy(\.isHexDigit) else { return nil }

        let hex6: String
        switch s.count {
        case 3: hex6 = s.map { "\($0)\($0)" }.joined()
        case 6, 8: hex6 = String(s.prefix(6))
        default: return nil
        }
        guard let v = UInt32(hex6, radix: 16) else { return nil }
        self.init(r: UInt8(truncatingIfNeeded: v >> 16), g: UInt8(truncatingIfNeeded: v >> 8), b: UInt8(truncatingIfNeeded: v))
    }

    public var hex: String { String(format: "#%02x%02x%02x", r, g, b) }

    public var description: String { hex }

    /// WCAG relative luminance, 0 (black) to 1 (white).
    public var relativeLuminance: Double {
        0.2126 * Self.linear(r) + 0.7152 * Self.linear(g) + 0.0722 * Self.linear(b)
    }

    /// True for colors that read as a light background.
    public var isLight: Bool { relativeLuminance > 0.4 }

    private static func linear(_ channel: UInt8) -> Double {
        let c = Double(channel) / 255
        return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4)
    }
}

extension RgbColor: Codable {
    public init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        let text = try container.decode(String.self)
        guard let color = RgbColor(hex: text) else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid color '\(text)'.")
        }
        self = color
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(hex)
    }
}
