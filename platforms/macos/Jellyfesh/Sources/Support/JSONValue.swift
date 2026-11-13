// A small JSON reader and writer that keeps the order of object members. Plugin settings forms
// list a schema's properties in the order the plugin wrote them; JSONSerialization returns
// dictionaries without order.

import Foundation

/// One member of a JSON object.
struct JSONMember: Hashable {
    var key: String
    var value: JSONValue
}

/// A JSON value; objects keep their members in document order.
enum JSONValue: Hashable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([JSONMember])

    /// Parses a whole JSON text.
    static func parse(_ text: String) throws -> JSONValue {
        var parser = JSONParser(Array(text.utf8))
        return try parser.document()
    }

    /// The member named `key` of an object.
    subscript(key: String) -> JSONValue? {
        guard case .object(let members) = self else { return nil }
        return members.first { $0.key == key }?.value
    }

    var stringValue: String? {
        if case .string(let value) = self { return value }
        return nil
    }

    var boolValue: Bool? {
        if case .bool(let value) = self { return value }
        return nil
    }

    var doubleValue: Double? {
        if case .number(let value) = self { return value }
        return nil
    }

    var arrayValue: [JSONValue]? {
        if case .array(let value) = self { return value }
        return nil
    }

    var members: [JSONMember]? {
        if case .object(let value) = self { return value }
        return nil
    }

    var isNull: Bool { self == .null }

    /// An object with `key` set to `value`: replaced in place, or added at the end.
    func setting(_ key: String, to value: JSONValue) -> JSONValue {
        var list = members ?? []
        if let index = list.firstIndex(where: { $0.key == key }) {
            list[index].value = value
        } else {
            list.append(JSONMember(key: key, value: value))
        }
        return .object(list)
    }

    /// Compact JSON text.
    var jsonText: String {
        switch self {
        case .null:
            return "null"
        case .bool(let value):
            return value ? "true" : "false"
        case .number(let value):
            guard value.isFinite else { return "null" }
            if value == value.rounded(), abs(value) < 9_007_199_254_740_992 {
                return String(Int64(value))
            }
            return String(value)
        case .string(let value):
            return Self.quoted(value)
        case .array(let values):
            return "[" + values.map(\.jsonText).joined(separator: ",") + "]"
        case .object(let members):
            return "{" + members.map { Self.quoted($0.key) + ":" + $0.value.jsonText }.joined(separator: ",") + "}"
        }
    }

    /// The text of a value for display: strings as they are, anything else as JSON.
    var displayText: String {
        stringValue ?? jsonText
    }

    private static func quoted(_ text: String) -> String {
        var out = "\""
        for scalar in text.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if scalar.value < 0x20 {
                    out += String(format: "\\u%04x", scalar.value)
                } else {
                    out.unicodeScalars.append(scalar)
                }
            }
        }
        return out + "\""
    }
}

enum JSONError: Error, Equatable {
    case unexpected(at: Int)
    case endOfInput
}

/// A recursive-descent parser over UTF-8 bytes.
private struct JSONParser {
    private let bytes: [UInt8]
    private var index = 0
    private var depth = 0

    init(_ bytes: [UInt8]) {
        self.bytes = bytes
    }

    mutating func document() throws -> JSONValue {
        let result = try value()
        skipSpace()
        guard index == bytes.count else { throw JSONError.unexpected(at: index) }
        return result
    }

    private mutating func value() throws -> JSONValue {
        skipSpace()
        guard index < bytes.count else { throw JSONError.endOfInput }
        switch bytes[index] {
        case UInt8(ascii: "{"): return try object()
        case UInt8(ascii: "["): return try array()
        case UInt8(ascii: "\""): return .string(try string())
        case UInt8(ascii: "t"): try literal("true"); return .bool(true)
        case UInt8(ascii: "f"): try literal("false"); return .bool(false)
        case UInt8(ascii: "n"): try literal("null"); return .null
        default: return .number(try number())
        }
    }

    private mutating func object() throws -> JSONValue {
        try enter()
        defer { depth -= 1 }
        index += 1
        var members: [JSONMember] = []
        skipSpace()
        if peek(UInt8(ascii: "}")) {
            index += 1
            return .object(members)
        }
        while true {
            skipSpace()
            guard peek(UInt8(ascii: "\"")) else { throw JSONError.unexpected(at: index) }
            let key = try string()
            skipSpace()
            try expect(UInt8(ascii: ":"))
            members.append(JSONMember(key: key, value: try value()))
            skipSpace()
            if peek(UInt8(ascii: ",")) {
                index += 1
                continue
            }
            try expect(UInt8(ascii: "}"))
            return .object(members)
        }
    }

    private mutating func array() throws -> JSONValue {
        try enter()
        defer { depth -= 1 }
        index += 1
        var values: [JSONValue] = []
        skipSpace()
        if peek(UInt8(ascii: "]")) {
            index += 1
            return .array(values)
        }
        while true {
            values.append(try value())
            skipSpace()
            if peek(UInt8(ascii: ",")) {
                index += 1
                continue
            }
            try expect(UInt8(ascii: "]"))
            return .array(values)
        }
    }

    private mutating func string() throws -> String {
        index += 1 // the opening quote
        var out: [UInt8] = []
        while index < bytes.count {
            let byte = bytes[index]
            index += 1
            switch byte {
            case UInt8(ascii: "\""):
                return String(decoding: out, as: UTF8.self)
            case UInt8(ascii: "\\"):
                guard index < bytes.count else { throw JSONError.endOfInput }
                let escape = bytes[index]
                index += 1
                switch escape {
                case UInt8(ascii: "\""): out.append(UInt8(ascii: "\""))
                case UInt8(ascii: "\\"): out.append(UInt8(ascii: "\\"))
                case UInt8(ascii: "/"): out.append(UInt8(ascii: "/"))
                case UInt8(ascii: "b"): out.append(0x08)
                case UInt8(ascii: "f"): out.append(0x0C)
                case UInt8(ascii: "n"): out.append(0x0A)
                case UInt8(ascii: "r"): out.append(0x0D)
                case UInt8(ascii: "t"): out.append(0x09)
                case UInt8(ascii: "u"):
                    var code = try hex4()
                    // A surrogate pair spells one scalar beyond the Basic Multilingual Plane.
                    if (0xD800...0xDBFF).contains(code), index + 1 < bytes.count,
                       bytes[index] == UInt8(ascii: "\\"), bytes[index + 1] == UInt8(ascii: "u") {
                        index += 2
                        let low = try hex4()
                        if (0xDC00...0xDFFF).contains(low) {
                            code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00)
                        }
                    }
                    let scalar = Unicode.Scalar(code) ?? "\u{FFFD}"
                    out.append(contentsOf: Array(String(Character(scalar)).utf8))
                default:
                    throw JSONError.unexpected(at: index - 1)
                }
            case 0x00..<0x20:
                throw JSONError.unexpected(at: index - 1)
            default:
                out.append(byte)
            }
        }
        throw JSONError.endOfInput
    }

    private mutating func hex4() throws -> UInt32 {
        guard index + 4 <= bytes.count else { throw JSONError.endOfInput }
        var code: UInt32 = 0
        for _ in 0..<4 {
            let byte = bytes[index]
            let digit: UInt32
            switch byte {
            case UInt8(ascii: "0")...UInt8(ascii: "9"): digit = UInt32(byte - UInt8(ascii: "0"))
            case UInt8(ascii: "a")...UInt8(ascii: "f"): digit = UInt32(byte - UInt8(ascii: "a")) + 10
            case UInt8(ascii: "A")...UInt8(ascii: "F"): digit = UInt32(byte - UInt8(ascii: "A")) + 10
            default: throw JSONError.unexpected(at: index)
            }
            code = code * 16 + digit
            index += 1
        }
        return code
    }

    private mutating func number() throws -> Double {
        let start = index
        func isNumberByte(_ byte: UInt8) -> Bool {
            switch byte {
            case UInt8(ascii: "0")...UInt8(ascii: "9"), UInt8(ascii: "-"), UInt8(ascii: "+"),
                 UInt8(ascii: "."), UInt8(ascii: "e"), UInt8(ascii: "E"):
                return true
            default:
                return false
            }
        }
        while index < bytes.count, isNumberByte(bytes[index]) {
            index += 1
        }
        let text = String(decoding: bytes[start..<index], as: UTF8.self)
        guard index > start, let value = Double(text) else { throw JSONError.unexpected(at: start) }
        return value
    }

    private mutating func literal(_ word: String) throws {
        let expected = Array(word.utf8)
        guard index + expected.count <= bytes.count,
              Array(bytes[index..<index + expected.count]) == expected
        else { throw JSONError.unexpected(at: index) }
        index += expected.count
    }

    private mutating func enter() throws {
        depth += 1
        // Settings schemas are shallow; a deep nesting is a broken or hostile file.
        guard depth <= 64 else { throw JSONError.unexpected(at: index) }
    }

    private mutating func expect(_ byte: UInt8) throws {
        guard peek(byte) else { throw index < bytes.count ? JSONError.unexpected(at: index) : JSONError.endOfInput }
        index += 1
    }

    private func peek(_ byte: UInt8) -> Bool {
        index < bytes.count && bytes[index] == byte
    }

    private mutating func skipSpace() {
        while index < bytes.count {
            switch bytes[index] {
            case 0x20, 0x09, 0x0A, 0x0D: index += 1
            default: return
            }
        }
    }
}
