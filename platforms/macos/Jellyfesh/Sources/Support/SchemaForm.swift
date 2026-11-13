// Plugin settings forms from the plugin's JSON Schema (ADR-0004: a subset of draft 2020-12,
// rendered natively), the same subset the Linux app reads (logic/schema.rs): an object whose
// `properties` are strings (`format` `password` or `uri`, or `writeOnly` for secrets), booleans,
// integers and numbers (`minimum`, `maximum`, `multipleOf`) and enums (labels from `enumNames`, or
// `oneOf` entries with `const` and `title`). Other property types are skipped; properties keep the
// schema's order. The core validates and stores what is sent.

import Foundation

/// One setting of a plugin.
struct SchemaField: Identifiable, Equatable {
    enum Kind: Equatable {
        case text(secret: Bool, url: Bool)
        case toggle
        case integer(min: Double, max: Double, step: Double)
        case number(min: Double, max: Double, step: Double)
        case choice([Choice])
    }

    struct Choice: Hashable {
        var value: JSONValue
        var label: String
    }

    var key: String
    var title: String
    var description: String?
    var kind: Kind
    var required: Bool
    var defaultValue: JSONValue?

    var id: String { key }
}

enum SchemaForm {
    /// The fields of a settings schema; an empty schema has none.
    static func fields(schemaJSON: String) throws -> [SchemaField] {
        guard !schemaJSON.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return [] }
        let schema = try JSONValue.parse(schemaJSON)
        let required = Set((schema["required"]?.arrayValue ?? []).compactMap(\.stringValue))
        var fields: [SchemaField] = []
        for member in schema["properties"]?.members ?? [] {
            let property = member.value
            guard let kind = kind(of: property) else { continue }
            fields.append(SchemaField(
                key: member.key,
                title: property["title"]?.stringValue ?? humanize(member.key),
                description: property["description"]?.stringValue,
                kind: kind,
                required: required.contains(member.key),
                defaultValue: property["default"]
            ))
        }
        return fields
    }

    /// The value of `field` in a plugin's settings JSON, else its default.
    static func current(settingsJSON: String, field: SchemaField) -> JSONValue? {
        if let settings = try? JSONValue.parse(settingsJSON), let value = settings[field.key], !value.isNull {
            return value
        }
        return field.defaultValue
    }

    /// The settings JSON with `key` set to `value`, other keys kept.
    static func withValue(settingsJSON: String, key: String, value: JSONValue) -> String {
        var settings = (try? JSONValue.parse(settingsJSON)) ?? .object([])
        if settings.members == nil {
            settings = .object([])
        }
        return settings.setting(key, to: value).jsonText
    }

    /// `server_url` → `Server url`.
    static func humanize(_ key: String) -> String {
        let spaced = key.replacingOccurrences(of: "_", with: " ").replacingOccurrences(of: "-", with: " ")
        guard let first = spaced.first else { return "" }
        return first.uppercased() + spaced.dropFirst()
    }

    private static func kind(of property: JSONValue) -> SchemaField.Kind? {
        if let options = choices(property) {
            return .choice(options)
        }
        func number(_ key: String) -> Double? { property[key]?.doubleValue }
        switch property["type"]?.stringValue {
        case "string"?:
            let format = property["format"]?.stringValue ?? ""
            let writeOnly = property["writeOnly"]?.boolValue ?? false
            return .text(secret: writeOnly || format == "password", url: format == "uri" || format == "url")
        case "boolean"?:
            return .toggle
        case "integer"?:
            return .integer(min: number("minimum") ?? 0, max: number("maximum") ?? 1_000_000,
                            step: max(number("multipleOf") ?? 1, 1))
        case "number"?:
            return .number(min: number("minimum") ?? -1_000_000, max: number("maximum") ?? 1_000_000,
                           step: number("multipleOf") ?? 0.1)
        default:
            return nil
        }
    }

    private static func choices(_ property: JSONValue) -> [SchemaField.Choice]? {
        if let values = property["enum"]?.arrayValue {
            let names = property["enumNames"]?.arrayValue
            return values.enumerated().map { index, value in
                let name = names.flatMap { index < $0.count ? $0[index].stringValue : nil }
                return SchemaField.Choice(value: value, label: name ?? value.displayText)
            }
        }
        guard let oneOf = property["oneOf"]?.arrayValue else { return nil }
        let options = oneOf.compactMap { option -> SchemaField.Choice? in
            guard let value = option["const"] else { return nil }
            return SchemaField.Choice(value: value, label: option["title"]?.stringValue ?? value.displayText)
        }
        return options.isEmpty ? nil : options
    }
}
