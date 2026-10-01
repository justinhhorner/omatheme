import Foundation

/// Reads a TOML file into "section.key" → string pairs (non-string values are dropped; palettes
/// only need strings). Keys are matched case-insensitively. Community theme files are often
/// hand-edited, so if strict parsing fails we fall back to a lenient line scanner rather than
/// rejecting the theme.
enum FlatToml {
    static func parse(_ text: String) -> [String: String] {
        var strict = StrictParser(text)
        return (try? strict.parse()) ?? parseLenient(text)
    }

    static func parseLenient(_ text: String) -> [String: String] {
        var result: [String: String] = [:]
        var section = ""
        // isNewline, not "\n": a CRLF line ending is a single Character in Swift.
        for rawLine in text.split(whereSeparator: \.isNewline) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }

            if let header = line.firstMatch(of: /^\[\s*([^\[\]]+?)\s*\]/) {
                section = header.1.replacing(" ", with: "").replacing("\"", with: "") + "."
                continue
            }
            if let kv = line.firstMatch(of: /^([A-Za-z0-9_\-]+)\s*=\s*["']([^"']*)["']/) {
                result[(section + kv.1).lowercased()] = String(kv.2)
            }
        }
        return result
    }

    struct SyntaxError: Error {}

    /// The subset of TOML 1.0 palette files use: tables, array tables, bare/quoted/dotted keys,
    /// basic and literal strings (single- and multi-line), inline tables and arrays. Other
    /// scalars (numbers, booleans, dates) are skipped. Duplicate keys are an error, as in TOML.
    private struct StrictParser {
        private let chars: [Character]
        private var i = 0
        private var result: [String: String] = [:]
        private var definedKeys: Set<String> = []

        init(_ text: String) {
            chars = Array(text.replacing("\r\n", with: "\n"))
        }

        mutating func parse() throws -> [String: String] {
            var table: [String] = []
            while true {
                skipWhitespaceAndComments(newlines: true)
                guard let c = peek else { return result }
                if c == "[" {
                    table = try parseTableHeader()
                } else {
                    try parseKeyValue(prefix: table)
                }
                try expectLineEnd()
            }
        }

        private var peek: Character? { i < chars.count ? chars[i] : nil }

        private func peek(_ offset: Int) -> Character? {
            i + offset < chars.count ? chars[i + offset] : nil
        }

        private mutating func skipWhitespaceAndComments(newlines: Bool) {
            while let c = peek {
                if c == " " || c == "\t" || (newlines && c == "\n") {
                    i += 1
                } else if c == "#" {
                    while let d = peek, d != "\n" { i += 1 }
                } else {
                    return
                }
            }
        }

        private mutating func expectLineEnd() throws {
            skipWhitespaceAndComments(newlines: false)
            guard peek == nil || peek == "\n" else { throw SyntaxError() }
        }

        private mutating func parseTableHeader() throws -> [String] {
            i += 1
            let isArray = peek == "["
            if isArray { i += 1 }
            skipWhitespaceAndComments(newlines: false)
            let key = try parseKey()
            skipWhitespaceAndComments(newlines: false)
            guard peek == "]" else { throw SyntaxError() }
            i += 1
            if isArray {
                guard peek == "]" else { throw SyntaxError() }
                i += 1
            }
            return key
        }

        /// A dotted key: `a.b`, `"quoted key".c`, `'literal'`.
        private mutating func parseKey() throws -> [String] {
            var parts: [String] = []
            while true {
                skipWhitespaceAndComments(newlines: false)
                switch peek {
                case "\"": parts.append(try parseBasicString())
                case "'": parts.append(try parseLiteralString())
                default:
                    let start = i
                    while let c = peek, c.isASCII, c.isLetter || c.isNumber || c == "_" || c == "-" { i += 1 }
                    guard i > start else { throw SyntaxError() }
                    parts.append(String(chars[start..<i]))
                }
                skipWhitespaceAndComments(newlines: false)
                guard peek == "." else { return parts }
                i += 1
            }
        }

        private mutating func parseKeyValue(prefix: [String]) throws {
            let key = prefix + (try parseKey())
            skipWhitespaceAndComments(newlines: false)
            guard peek == "=" else { throw SyntaxError() }
            i += 1
            skipWhitespaceAndComments(newlines: false)
            try parseValue(key: key)
        }

        private mutating func parseValue(key: [String]) throws {
            let flatKey = key.joined(separator: ".").lowercased()
            guard definedKeys.insert(flatKey).inserted else { throw SyntaxError() }
            switch peek {
            case "\"", "'": result[flatKey] = try parseString()
            case "{": try parseInlineTable(key: key)
            case "[": try skipArray()
            default: try skipScalar()
            }
        }

        private mutating func parseInlineTable(key: [String]) throws {
            i += 1
            skipWhitespaceAndComments(newlines: false)
            if peek == "}" { i += 1; return }
            while true {
                skipWhitespaceAndComments(newlines: false)
                try parseKeyValue(prefix: key)
                skipWhitespaceAndComments(newlines: false)
                if peek == "," { i += 1; continue }
                guard peek == "}" else { throw SyntaxError() }
                i += 1
                return
            }
        }

        private mutating func skipArray() throws {
            i += 1
            while true {
                skipWhitespaceAndComments(newlines: true)
                switch peek {
                case nil: throw SyntaxError()
                case "]": i += 1; return
                case ",": i += 1
                case "\"", "'": _ = try parseString()
                case "[": try skipArray()
                case "{": try parseInlineTable(key: ["\u{0}array\(i)"]) // array items aren't addressable keys
                default: try skipScalar()
                }
            }
        }

        /// Numbers, booleans and dates. Only their character set is checked, not their grammar.
        private mutating func skipScalar() throws {
            let start = i
            while let c = peek, c.isASCII, c.isLetter || c.isNumber || "+-_.:".contains(c) || (c == " " && peek(1)?.isNumber == true) {
                i += 1
            }
            guard i > start else { throw SyntaxError() }
        }

        private mutating func parseString() throws -> String {
            if peek == "\"" {
                return peek(1) == "\"" && peek(2) == "\"" ? try parseMultilineString(quote: "\"") : try parseBasicString()
            }
            return peek(1) == "'" && peek(2) == "'" ? try parseMultilineString(quote: "'") : try parseLiteralString()
        }

        private mutating func parseBasicString() throws -> String {
            i += 1
            var s = ""
            while let c = peek {
                i += 1
                switch c {
                case "\"": return s
                case "\n": throw SyntaxError()
                case "\\": s.append(try parseEscape())
                default: s.append(c)
                }
            }
            throw SyntaxError()
        }

        private mutating func parseLiteralString() throws -> String {
            i += 1
            let start = i
            while let c = peek {
                if c == "'" {
                    defer { i += 1 }
                    return String(chars[start..<i])
                }
                if c == "\n" { throw SyntaxError() }
                i += 1
            }
            throw SyntaxError()
        }

        private mutating func parseMultilineString(quote: Character) throws -> String {
            i += 3
            if peek == "\n" { i += 1 }
            var s = ""
            while let c = peek {
                if c == quote, peek(1) == quote, peek(2) == quote {
                    i += 3
                    return s
                }
                i += 1
                if quote == "\"" && c == "\\" {
                    if peek == "\n" || peek == " " || peek == "\t" {
                        // Line-ending backslash: trim the newline and following whitespace.
                        while let d = peek, d == " " || d == "\t" || d == "\n" { i += 1 }
                    } else {
                        s.append(try parseEscape())
                    }
                } else {
                    s.append(c)
                }
            }
            throw SyntaxError()
        }

        private mutating func parseEscape() throws -> Character {
            guard let c = peek else { throw SyntaxError() }
            i += 1
            switch c {
            case "b": return "\u{08}"
            case "t": return "\t"
            case "n": return "\n"
            case "f": return "\u{0C}"
            case "r": return "\r"
            case "e": return "\u{1B}"
            case "\"": return "\""
            case "\\": return "\\"
            case "u", "U":
                let length = c == "u" ? 4 : 8
                guard i + length <= chars.count,
                      let value = UInt32(String(chars[i..<(i + length)]), radix: 16),
                      let scalar = Unicode.Scalar(value)
                else { throw SyntaxError() }
                i += length
                return Character(scalar)
            default:
                throw SyntaxError()
            }
        }
    }
}
