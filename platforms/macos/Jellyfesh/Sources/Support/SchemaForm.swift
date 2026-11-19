// Plugin settings forms from the plugin's JSON Schema (ADR-0004: a subset of draft 2020-12,
// rendered natively), the same subset the Linux app reads (logic/schema.rs): an object whose
// `properties` are strings (`format` `password` or `uri`, or `writeOnly` for secrets), booleans,
// integers and numbers (`minimum`, `maximum`, `multipleOf`) and enums (labels from `enumNames`, or
// `oneOf` entries with `const` and `title`). Profile 1.2 (ADR-0020) adds colours (`"format":
// "color"`, `#RRGGBB`), sliders (numbers with both `minimum` and `maximum`, `multipleOf` the step),
// ordered checklists (arrays of choices with `uniqueItems`, whose value lists the shown items in
// order) and `x-jf-group` headings for consecutive properties. Other property types are skipped;
// properties keep the schema's order. The core validates and stores what is sent.

import Foundation

/// One setting of a plugin.
struct SchemaField: Identifiable, Equatable {
    enum Kind: Equatable {
        case text(secret: Bool, url: Bool)
        case toggle
        case integer(min: Double, max: Double, step: Double)
        case number(min: Double, max: Double, step: Double)
        /// A number or integer with both bounds: a slider, stepped by `multipleOf` when given.
        case slider(min: Double, max: Double, step: Double?, integer: Bool)
        case choice([Choice])
        /// `#RRGGBB` in a colour well.
        case color
        /// The shown choices, in order.
        case checklist([Choice])
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
    /// Its heading (`x-jf-group`): consecutive fields with the same one show under it.
    var group: String? = nil

    var id: String { key }
}

/// One row of an ordered checklist.
struct ChecklistRow: Hashable {
    var choice: SchemaField.Choice
    var shown: Bool
}

/// Consecutive fields under one heading (none for fields without a group).
struct SchemaGroup: Identifiable, Equatable {
    var title: String?
    var fields: [SchemaField]

    var id: String { fields.first?.key ?? title ?? "" }
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
                defaultValue: property["default"],
                group: property["x-jf-group"]?.stringValue.flatMap { $0.isEmpty ? nil : $0 }
            ))
        }
        return fields
    }

    /// The fields under their headings: consecutive fields with the same group together.
    static func groups(_ fields: [SchemaField]) -> [SchemaGroup] {
        var groups: [SchemaGroup] = []
        for field in fields {
            if let last = groups.last, last.title == field.group {
                groups[groups.count - 1].fields.append(field)
            } else {
                groups.append(SchemaGroup(title: field.group, fields: [field]))
            }
        }
        return groups
    }

    // MARK: Ordered checklists

    /// A checklist's rows: the shown items in the value's order, then the others in the schema's
    /// order. Values the schema does not offer are left out.
    static func checklistRows(options: [SchemaField.Choice], value: JSONValue?) -> [ChecklistRow] {
        let shown = shownItems(options: options, value: value)
        let hidden = options.filter { option in !shown.contains { $0.value == option.value } }
        return shown.map { ChecklistRow(choice: $0, shown: true) } + hidden.map { ChecklistRow(choice: $0, shown: false) }
    }

    /// The value with `item` shown (at the end) or hidden.
    static func checklistValue(options: [SchemaField.Choice], value: JSONValue?, item: JSONValue,
                               shown: Bool) -> JSONValue {
        var items = shownItems(options: options, value: value).map(\.value)
        items.removeAll { $0 == item }
        if shown, options.contains(where: { $0.value == item }) {
            items.append(item)
        }
        return .array(items)
    }

    /// The value with the shown `item` moved `offset` places (−1 up, +1 down), within the list.
    static func checklistValue(options: [SchemaField.Choice], value: JSONValue?, moving item: JSONValue,
                               by offset: Int) -> JSONValue {
        var items = shownItems(options: options, value: value).map(\.value)
        guard let from = items.firstIndex(of: item) else { return .array(items) }
        let to = min(max(from + offset, 0), items.count - 1)
        items.insert(items.remove(at: from), at: to)
        return .array(items)
    }

    private static func shownItems(options: [SchemaField.Choice], value: JSONValue?) -> [SchemaField.Choice] {
        var seen: [JSONValue] = []
        var shown: [SchemaField.Choice] = []
        for item in value?.arrayValue ?? [] where !seen.contains(item) {
            seen.append(item)
            if let option = options.first(where: { $0.value == item }) {
                shown.append(option)
            }
        }
        return shown
    }

    // MARK: Colours

    /// `#RRGGBB` as 0xRRGGBB; nil for anything else.
    static func rgb(hex: String) -> UInt32? {
        let text = hex.trimmingCharacters(in: .whitespaces)
        guard text.count == 7, text.hasPrefix("#") else { return nil }
        let digits = text.dropFirst()
        guard digits.allSatisfy(\.isHexDigit) else { return nil }
        return UInt32(digits, radix: 16)
    }

    /// 0xRRGGBB as `#RRGGBB`.
    static func hex(rgb: UInt32) -> String {
        "#" + String(format: "%06X", rgb & 0xFF_FFFF)
    }

    /// sRGB components (0–1) as 0xRRGGBB.
    static func rgb(red: Double, green: Double, blue: Double) -> UInt32 {
        func byte(_ value: Double) -> UInt32 {
            guard value.isFinite else { return 0 }
            return UInt32((min(max(value, 0), 1) * 255).rounded())
        }
        return (byte(red) << 16) | (byte(green) << 8) | byte(blue)
    }

    // MARK: Sliders

    /// The decimal places a slider's value shows: none for integers, as many as its step has, two
    /// at most (two without a step).
    static func fractionDigits(step: Double?, integer: Bool) -> Int {
        if integer { return 0 }
        guard let step, step > 0, step.isFinite else { return 2 }
        for digits in 0..<2 {
            let scaled = step * pow(10, Double(digits))
            if abs(scaled - scaled.rounded()) < 1e-9 { return digits }
        }
        return 2
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
        func number(_ key: String) -> Double? { property[key]?.doubleValue }
        let type = property["type"]?.stringValue
        if type == "array" {
            guard property["uniqueItems"]?.boolValue == true, let items = property["items"],
                  let options = choices(items)
            else { return nil }
            return .checklist(options)
        }
        if let options = choices(property) {
            return .choice(options)
        }
        switch type {
        case "string"?:
            let format = property["format"]?.stringValue ?? ""
            if format == "color" {
                return .color
            }
            let writeOnly = property["writeOnly"]?.boolValue ?? false
            return .text(secret: writeOnly || format == "password", url: format == "uri" || format == "url")
        case "boolean"?:
            return .toggle
        case "integer"?, "number"?:
            let integer = type == "integer"
            if let low = number("minimum"), let high = number("maximum"), high > low {
                let step = number("multipleOf").flatMap { $0 > 0 ? $0 : nil }
                return .slider(min: low, max: high, step: integer ? max(step ?? 1, 1) : step, integer: integer)
            }
            if integer {
                return .integer(min: number("minimum") ?? 0, max: number("maximum") ?? 1_000_000,
                                step: max(number("multipleOf") ?? 1, 1))
            }
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
