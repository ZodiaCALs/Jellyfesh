// A plugin's settings, rendered natively from its JSON Schema (SchemaForm) in the app's language
// (the core's localised schema, API 1.2): text fields, switches, steppers, choices, and from
// profile 1.2 sliders, colour wells and ordered checklists, under their `x-jf-group` headings.
// Text is sent on Return, when its field loses focus and before a sign-in starts; sliders when they
// are let go; colours once the colour panel rests; everything else at once. Secrets never show the
// stored value.

import SwiftUI

struct PluginSettingsForm: View {
    var plugin: SourcePlugin

    @Environment(SourcesStore.self) private var sources

    var body: some View {
        Group {
            switch Result(catching: { try SchemaForm.fields(schemaJSON: sources.settingsSchema(of: plugin)) }) {
            case .success(let fields):
                ForEach(SchemaForm.groups(fields)) { group in
                    if let title = group.title {
                        Text(title)
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .padding(.top, Tokens.Space.s1)
                            .accessibilityAddTraits(.isHeader)
                    }
                    ForEach(group.fields) { field in
                        SchemaFieldRow(plugin: plugin, field: field)
                    }
                }
            case .failure(let error):
                Text("This plugin's settings could not be read: \(String(describing: error))")
                    .foregroundStyle(.secondary)
            }
        }
        .task(id: plugin.version) {
            await sources.loadSettingsSchema(of: plugin,
                                             language: SourcesStore.schemaLanguage(preferred: Bundle.main.preferredLocalizations))
        }
    }
}

/// One setting.
private struct SchemaFieldRow: View {
    var plugin: SourcePlugin
    var field: SchemaField

    @Environment(SourcesStore.self) private var sources
    @State private var text = ""
    @State private var number: Double = 0
    @State private var loaded = false
    @FocusState private var focused: Bool

    var body: some View {
        let current = SchemaForm.current(settingsJSON: plugin.settingsJSON, field: field)
        Group {
            switch field.kind {
            case .text(let secret, _):
                textField(secret: secret)
            case .toggle:
                Toggle(field.title, isOn: Binding(get: { current?.boolValue ?? false }, set: { save(.bool($0)) }))
            case .integer(let low, let high, let step):
                let value = current?.doubleValue ?? low
                Stepper(value: Binding(get: { value }, set: { save(.number($0.rounded())) }),
                        in: low...Swift.max(low, high), step: step) {
                    LabeledContent(field.title) {
                        Text(verbatim: "\(Int(value))")
                            .monospacedDigit()
                    }
                }
            case .number(let low, let high, _):
                LabeledContent(field.title) {
                    TextField("", value: $number, format: .number)
                        .multilineTextAlignment(.trailing)
                        .frame(maxWidth: 120)
                        .onSubmit {
                            save(.number(Swift.min(Swift.max(number, low), Swift.max(low, high))))
                        }
                }
            case .slider(let low, let high, let step, let integer):
                SchemaSlider(title: field.title, value: current?.doubleValue ?? low, range: low...high, step: step,
                             integer: integer) { value in
                    save(.number(value))
                }
            case .choice(let options):
                Picker(field.title, selection: Binding<Int>(
                    get: { options.firstIndex { $0.value == current } ?? -1 },
                    set: { index in
                        if options.indices.contains(index) { save(options[index].value) }
                    }
                )) {
                    if current == nil {
                        Text("Choose…").tag(-1)
                    }
                    ForEach(Array(options.enumerated()), id: \.offset) { index, option in
                        Text(option.label).tag(index)
                    }
                }
            case .color:
                SchemaColorWell(title: field.title, rgb: current?.stringValue.flatMap(SchemaForm.rgb(hex:))) { rgb in
                    save(.string(SchemaForm.hex(rgb: rgb)))
                }
            case .checklist(let options):
                SchemaChecklist(field: field, options: options, value: current) { value in
                    save(value)
                }
            }
        }
        .modifier(OptionalHelp(text: field.description))
        .onAppear {
            guard !loaded else { return }
            loaded = true
            if case .text(let secret, _) = field.kind, !secret {
                text = current?.stringValue ?? ""
            }
            number = current?.doubleValue ?? 0
        }
        .onChange(of: focused) { _, isFocused in
            if !isFocused { sources.commitSettings(plugin.id) }
        }
        .onDisappear { sources.commitSettings(plugin.id) }
    }

    /// A text setting in a bordered field, so an empty one still shows where to type, with the
    /// plugin's description under its name (where it gives an example address, for one).
    @ViewBuilder private func textField(secret: Bool) -> some View {
        let binding = Binding(get: { text }, set: { value in
            text = value
            sources.editSetting(plugin.id, key: field.key, value: .string(value))
        })
        Group {
            if secret {
                SecureField(text: binding, prompt: nil) { fieldLabel }
            } else {
                TextField(text: binding, prompt: nil) { fieldLabel }
            }
        }
        .textFieldStyle(.roundedBorder)
        .focused($focused)
        .onSubmit { sources.commitSettings(plugin.id) }
    }

    @ViewBuilder private var fieldLabel: some View {
        Text(field.title)
        if let description = field.description, !description.isEmpty {
            Text(description)
        }
    }

    private func save(_ value: JSONValue) {
        sources.setSetting(plugin.id, key: field.key, value: value)
    }
}

/// A number between two bounds: the value beside the slider, sent when the slider is let go.
private struct SchemaSlider: View {
    var title: String
    var value: Double
    var range: ClosedRange<Double>
    var step: Double?
    var integer: Bool
    var onCommit: (Double) -> Void

    @State private var draft: Double?
    /// The value sent, shown until the core reports it.
    @State private var sent: Double?

    var body: some View {
        let shown = draft ?? sent ?? value
        LabeledContent {
            HStack(spacing: Tokens.Space.s3) {
                slider
                    .frame(minWidth: 160)
                Text(shown, format: .number.precision(.fractionLength(SchemaForm.fractionDigits(step: step,
                                                                                               integer: integer))))
                    .monospacedDigit()
                    .frame(minWidth: 40, alignment: .trailing)
            }
        } label: {
            Text(title)
        }
        .onChange(of: value) { _, _ in
            sent = nil
        }
    }

    @ViewBuilder private var slider: some View {
        let binding = Binding(get: { Swift.min(Swift.max(draft ?? sent ?? value, range.lowerBound), range.upperBound) },
                              set: { draft = $0 })
        if let step, step > 0 {
            Slider(value: binding, in: range, step: step, onEditingChanged: finish)
                .accessibilityLabel(Text(title))
        } else {
            Slider(value: binding, in: range, onEditingChanged: finish)
                .accessibilityLabel(Text(title))
        }
    }

    private func finish(_ editing: Bool) {
        guard !editing, let chosen = draft else { return }
        let committed = integer ? chosen.rounded() : chosen
        draft = nil
        if committed != value {
            sent = committed
            onCommit(committed)
        }
    }
}

/// `#RRGGBB` in a colour well. The colour panel reports every move; the setting is sent once it
/// rests for a moment.
private struct SchemaColorWell: View {
    var title: String
    var rgb: UInt32?
    var onCommit: (UInt32) -> Void

    @State private var draft: UInt32?

    var body: some View {
        ColorPicker(selection: Binding(get: { Self.color(draft ?? rgb ?? 0x80_80_80) },
                                       set: { draft = Self.rgb(of: $0) }),
                    supportsOpacity: false) {
            Text(title)
        }
        .task(id: draft) {
            guard let chosen = draft, chosen != rgb else { return }
            try? await Task.sleep(nanoseconds: 300_000_000)
            guard !Task.isCancelled else { return }
            onCommit(chosen)
        }
        .onChange(of: rgb) { _, now in
            // The core has it: show what it reports again.
            if now == draft { draft = nil }
        }
    }

    private static func color(_ rgb: UInt32) -> CGColor {
        CGColor(srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255, green: CGFloat((rgb >> 8) & 0xFF) / 255,
                blue: CGFloat(rgb & 0xFF) / 255, alpha: 1)
    }

    private static func rgb(of color: CGColor) -> UInt32? {
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let srgb = color.converted(to: space, intent: .defaultIntent, options: nil),
              let parts = srgb.components, parts.count >= 3
        else { return nil }
        return SchemaForm.rgb(red: Double(parts[0]), green: Double(parts[1]), blue: Double(parts[2]))
    }
}

/// An ordered checklist: every choice with a checkbox, the shown ones first in their order, each
/// moved up or down with its buttons.
private struct SchemaChecklist: View {
    var field: SchemaField
    var options: [SchemaField.Choice]
    var value: JSONValue?
    var onChange: (JSONValue) -> Void

    var body: some View {
        let rows = SchemaForm.checklistRows(options: options, value: value)
        let shown = rows.filter { $0.shown }.count
        VStack(alignment: .leading, spacing: Tokens.Space.s1) {
            Text(field.title)
            if let description = field.description, !description.isEmpty {
                Text(description)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
            ForEach(Array(rows.enumerated()), id: \.element.choice) { index, row in
                HStack(spacing: Tokens.Space.s2) {
                    Toggle(row.choice.label, isOn: Binding(get: { row.shown }, set: { on in
                        onChange(SchemaForm.checklistValue(options: options, value: value, item: row.choice.value,
                                                           shown: on))
                    }))
                    .toggleStyle(.checkbox)
                    Spacer(minLength: 0)
                    if row.shown {
                        move(row.choice, by: -1, symbol: "chevron.up", label: "Move Up")
                            .disabled(index == 0)
                        move(row.choice, by: 1, symbol: "chevron.down", label: "Move Down")
                            .disabled(index == shown - 1)
                    }
                }
            }
        }
    }

    private func move(_ choice: SchemaField.Choice, by offset: Int, symbol: String,
                      label: LocalizedStringKey) -> some View {
        Button {
            onChange(SchemaForm.checklistValue(options: options, value: value, moving: choice.value, by: offset))
        } label: {
            Image(systemName: symbol)
        }
        .buttonStyle(.borderless)
        .accessibilityLabel(Text(label))
        .help(Text(label))
    }
}

/// A tooltip only when there is text for it.
private struct OptionalHelp: ViewModifier {
    var text: String?

    func body(content: Content) -> some View {
        if let text, !text.isEmpty {
            content.help(text)
        } else {
            content
        }
    }
}
