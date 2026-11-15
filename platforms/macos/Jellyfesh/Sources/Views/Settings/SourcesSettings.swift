// Sources, in the order of docs/ui.md: the services to add, each connected from a sheet with its
// server's address and a sign-in method; every installed plugin with its badge, enabling, account
// and sign-in flows, settings from its JSON Schema and uninstalling, with installing a `.jfpkg`
// file in the heading; and the community-plugin switch with ADR-0012's disclosure.

import SwiftUI

struct SourcesSettings: View {
    @Environment(SourcesStore.self) private var sources
    @Environment(SettingsStore.self) private var settings
    @State private var disclosure: CommunityDisclosure?
    /// The service whose Connect sheet is open.
    @State private var connecting: SourcePreset?

    var body: some View {
        Form {
            if sources.unavailable {
                Section {
                    SourcesUnavailableNotice()
                }
            }

            if !sources.presets.isEmpty {
                Section {
                    ForEach(sources.presets) { preset in
                        PresetRow(preset: preset, plugin: sources.plugin(preset.plugin)) {
                            connecting = preset
                        }
                    }
                } header: {
                    Text("Add a Source")
                }
            }

            Section {
                if sources.plugins.isEmpty {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("No Sources Installed")
                        Text("Install a source plugin from a .jfpkg file.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
                ForEach(sources.plugins) { plugin in
                    PluginSettingsRow(plugin: plugin, allowCommunity: settings.current.allowCommunityPlugins,
                                      disclosure: $disclosure)
                }
            } header: {
                HStack {
                    Text("Installed")
                    Spacer()
                    Button("Install from File…") {
                        FilePanels.choosePlugin { url in sources.install(path: url.path) }
                    }
                    .buttonStyle(.link)
                }
            }

            Section {
                Toggle(isOn: Binding(
                    get: { settings.current.allowCommunityPlugins },
                    set: { allow in
                        if allow {
                            // Turning it on needs the disclosure first.
                            disclosure = CommunityDisclosure(pluginID: nil, pluginName: nil)
                        } else {
                            settings.setAllowCommunityPlugins(false)
                        }
                    }
                )) {
                    Text("Allow Community Plugins")
                    Text("Official sources are signed by Jellyfesh. Community plugins are made by others and are off unless you allow them.")
                }
            } header: {
                Text("Community Plugins")
            }
        }
        .formStyle(.grouped)
        .communityDisclosure($disclosure) { request in
            if let id = request.pluginID {
                sources.acknowledgeAndEnable(id)
            } else {
                settings.setAllowCommunityPlugins(true)
            }
        }
        .connectSourceFlow($connecting, in: .settings)
    }
}

extension View {
    /// Add a Source's steps, for a view that lists the services: the Connect sheet of the service
    /// in `connecting`, then the sheet of the sign-in flow it starts. Every flow started in
    /// `window` (an installed source's sign-in buttons too) shows its sheet here.
    func connectSourceFlow(_ connecting: Binding<SourcePreset?>, in window: SignInWindow) -> some View {
        modifier(ConnectSourceFlow(connecting: connecting, window: window))
    }
}

private struct ConnectSourceFlow: ViewModifier {
    @Binding var connecting: SourcePreset?
    var window: SignInWindow

    @Environment(SourcesStore.self) private var sources
    /// What was chosen in the Connect sheet, acted on once it has gone (`connectChosen`).
    @State private var chosen: ConnectChoice?

    func body(content: Content) -> some View {
        content
            .sheet(item: $connecting, onDismiss: { connectChosen() }) { preset in
                ConnectSourceSheet(preset: preset) { address, method in
                    chosen = ConnectChoice(preset: preset, address: address, method: method)
                }
            }
            .sheet(item: Binding(get: { sources.signInFlow(in: window) }, set: { flow in
                if flow == nil { sources.endSignIn() }
            })) { _ in
                SignInSheet()
            }
    }

    /// Starts the sign-in chosen in the Connect sheet. Only once that sheet has gone: a window
    /// shows one sheet at a time, and the sign-in sheet opens as soon as the flow starts.
    private func connectChosen() {
        guard let choice = chosen else { return }
        chosen = nil
        sources.connect(choice.preset, address: choice.address, method: choice.method, in: window)
    }
}

/// A sign-in method chosen in the Connect sheet, with the address typed there.
private struct ConnectChoice {
    var preset: SourcePreset
    var address: String
    var method: SignInMethod
}

/// A service in Add a Source (Settings and the Sources page): its plugin's icon, its name, and what
/// it is or its account.
struct PresetRow: View {
    var preset: SourcePreset
    var plugin: SourcePlugin?
    var iconSize: CGFloat = 28
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: Tokens.Space.s3) {
                PluginIcon(plugin: plugin, size: iconSize)
                VStack(alignment: .leading, spacing: 1) {
                    Text(preset.name)
                    if let detail = SourceText.detail(preset) {
                        Text(detail)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(.tertiary)
                    .accessibilityHidden(true)
            }
            // The whole row answers a click, the space between included.
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// Connecting to a service: its server's address when it needs one, then a button per sign-in
/// method. `choose` gets the address and the method as the sheet closes; the sign-in sheet takes
/// over from there.
private struct ConnectSourceSheet: View {
    var preset: SourcePreset
    var choose: (String, SignInMethod) -> Void

    @Environment(SourcesStore.self) private var sources
    @Environment(\.dismiss) private var dismiss
    @State private var address: String

    init(preset: SourcePreset, choose: @escaping (String, SignInMethod) -> Void) {
        self.preset = preset
        self.choose = choose
        _address = State(initialValue: preset.address ?? "")
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s5) {
            Text("Connect to \(preset.name)")
                .font(.system(size: 20, weight: .bold))
                .accessibilityAddTraits(.isHeader)
            if preset.needsAddress {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    // Outside a form a field shows only its prompt, so the label goes above it; the
                    // field keeps its own for VoiceOver.
                    Text("Server Address")
                        .accessibilityHidden(true)
                    TextField("Server Address", text: $address, prompt: preset.addressExample.map { Text($0) })
                        .textFieldStyle(.roundedBorder)
                        .labelsHidden()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack {
                Spacer()
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.cancelAction)
                ForEach(preset.signInMethods, id: \.self) { method in
                    methodButton(method)
                }
            }
        }
        .padding(Tokens.Space.s6)
        .frame(width: 440)
    }

    /// One sign-in method, once the address is there. Signing in with a password is the default.
    @ViewBuilder private func methodButton(_ method: SignInMethod) -> some View {
        let button = Button(PluginSettingsRow.title(of: method)) {
            choose(address, method)
            dismiss()
        }
        .disabled(!sources.canConnect(preset, address: address))
        if method == .passwordExchange {
            button.keyboardShortcut(.defaultAction)
        } else {
            button
        }
    }
}

/// One installed plugin, expandable to its settings.
private struct PluginSettingsRow: View {
    var plugin: SourcePlugin
    var allowCommunity: Bool
    @Binding var disclosure: CommunityDisclosure?

    @Environment(SourcesStore.self) private var sources
    @State private var expanded = false
    @State private var confirmUninstall = false

    var body: some View {
        DisclosureGroup(isExpanded: $expanded) {
            Toggle("Enabled", isOn: Binding(get: { plugin.enabled }, set: { on in
                if on && plugin.needsDisclosure {
                    disclosure = CommunityDisclosure(pluginID: plugin.id, pluginName: plugin.name)
                } else {
                    sources.setEnabled(plugin.id, on)
                }
            }))
            .disabled(plugin.badge == .community && !allowCommunity)

            LabeledContent("Account") {
                account
            }

            PluginSettingsForm(plugin: plugin)

            HStack {
                Text(CountText.joined([String(localized: "Version \(plugin.version)"), plugin.author]))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Uninstall…", role: .destructive) {
                    confirmUninstall = true
                }
            }
        } label: {
            HStack(spacing: Tokens.Space.s3) {
                PluginIcon(plugin: plugin, size: 28)
                VStack(alignment: .leading, spacing: 1) {
                    Text(plugin.name)
                    Text(CountText.joined([SourceText.badge(plugin), SourceText.account(plugin)]))
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                BadgeLabel(badge: plugin.badge)
            }
        }
        .confirmationDialog(Text("Uninstall \(plugin.name)?"), isPresented: $confirmUninstall,
                            titleVisibility: .visible) {
            Button("Uninstall", role: .destructive) {
                sources.uninstall(plugin.id)
            }
        } message: {
            Text("Its settings and sign-in are removed.")
        }
    }

    @ViewBuilder private var account: some View {
        if let name = plugin.account {
            HStack {
                Text("Signed in as \(name)")
                    .foregroundStyle(.secondary)
                Button("Sign Out") {
                    sources.logout(plugin.id)
                }
            }
        } else if plugin.signInMethods.isEmpty {
            Text("No sign-in needed")
                .foregroundStyle(.secondary)
        } else {
            HStack {
                ForEach(plugin.signInMethods, id: \.self) { method in
                    Button(Self.title(of: method)) {
                        sources.beginSignIn(plugin, method: method)
                    }
                    .disabled(!plugin.enabled)
                }
            }
        }
    }

    /// A sign-in method's button, here and in the Connect sheet.
    static func title(of method: SignInMethod) -> LocalizedStringKey {
        switch method {
        case .qr: return "Sign In with QR Code"
        case .oauth: return "Sign In in Browser"
        case .cookieImport: return "Paste Cookies"
        case .passwordExchange: return "Sign In"
        case .apiKey: return "Enter API Key"
        }
    }
}

/// Official or Community, as a small capsule.
private struct BadgeLabel: View {
    var badge: PluginBadgeKind
    @Environment(\.palette) private var palette

    var body: some View {
        Text(badge == .official ? LocalizedStringKey("Official") : LocalizedStringKey("Community"))
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(badge == .official ? palette[Tokens.Accent.accentText] : palette[Tokens.Semantic.warning])
            .padding(.horizontal, 8)
            .padding(.vertical, 2)
            .background(Capsule().fill(badge == .official ? palette[Tokens.Accent.accentSoft] : palette[Tokens.Ink.ink06]))
    }
}

/// A plugin's settings, rendered from its JSON Schema (SchemaForm).
struct PluginSettingsForm: View {
    var plugin: SourcePlugin

    var body: some View {
        switch Result(catching: { try SchemaForm.fields(schemaJSON: plugin.settingsSchemaJSON) }) {
        case .success(let fields):
            ForEach(fields) { field in
                SchemaFieldRow(plugin: plugin, field: field)
            }
        case .failure(let error):
            Text("This plugin's settings could not be read: \(String(describing: error))")
                .foregroundStyle(.secondary)
        }
    }
}

/// One setting. Text is sent on Return, when its field loses focus and before a sign-in starts;
/// toggles, steppers and choices at once. Secrets never show the stored value.
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
