import Foundation

/// Shared JSON settings and crash-safe read/write helpers for the app's local files. The format is
/// the contract in docs/data-format.md, shared with the Windows app: camelCase keys, absent values
/// omitted, UTC dates with milliseconds and "Z".
public enum JSONFile {
    public static func makeEncoder() -> JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        encoder.dateEncodingStrategy = .custom { date, encoder in
            var container = encoder.singleValueContainer()
            try container.encode(formatDate(date))
        }
        return encoder
    }

    public static func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let text = try container.decode(String.self)
            guard let date = parseDate(text) else {
                throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid date '\(text)'.")
            }
            return date
        }
        return decoder
    }

    /// "2026-09-27T09:00:00.123Z": UTC, milliseconds. Rounds to the nearest millisecond first: the
    /// formatter truncates, and a Date's floating-point seconds can sit just below the value
    /// (…600.123 is stored as …600.12299…).
    static func formatDate(_ date: Date) -> String {
        let milliseconds = Int64((date.timeIntervalSince1970 * 1000).rounded())
        let (seconds, fraction) = milliseconds.quotientAndRemainder(dividingBy: 1000)
        let (wholeSeconds, millis) = fraction < 0 ? (seconds - 1, fraction + 1000) : (seconds, fraction)
        let base = Date(timeIntervalSince1970: TimeInterval(wholeSeconds)).formatted(Date.ISO8601FormatStyle()) // "…:00Z"
        return base.dropLast() + String(format: ".%03lldZ", millis)
    }

    /// Any ISO 8601 date-time: any fraction length or none, "Z", a "+hh:mm"/"+hhmm" offset, or no zone
    /// at all (older macOS files, which meant UTC).
    static func parseDate(_ text: String) -> Date? {
        guard let match = text.wholeMatch(of: /(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d+))?(Z|[+-]\d{2}:?\d{2})?/) else {
            return nil
        }
        let fraction = String((match.2.map(String.init) ?? "").prefix(3)).padding(toLength: 3, withPad: "0", startingAt: 0)
        guard let utc = try? Date("\(match.1).\(fraction)Z", strategy: Date.ISO8601FormatStyle(includingFractionalSeconds: true)) else {
            return nil
        }
        guard let zone = match.3, zone != "Z" else { return utc }
        let digits = zone.dropFirst().replacing(":", with: "")
        let offset = (Int(digits.prefix(2)) ?? 0) * 3600 + (Int(digits.suffix(2)) ?? 0) * 60
        return utc.addingTimeInterval(TimeInterval(zone.hasPrefix("-") ? offset : -offset))
    }

    /// Reads `url`, returning nil if it is missing or unreadable.
    public static func read<T: Decodable>(_ type: T.Type, from url: URL) -> T? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? makeDecoder().decode(type, from: data)
    }

    /// Writes via a temp file + rename so a crash never leaves a half-written file.
    public static func write<T: Encodable>(_ value: T, to url: URL) throws {
        try writeAtomic(makeEncoder().encode(value), to: url)
    }

    public static func writeAtomic(_ data: Data, to url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try data.write(to: url, options: .atomic)
    }
}
