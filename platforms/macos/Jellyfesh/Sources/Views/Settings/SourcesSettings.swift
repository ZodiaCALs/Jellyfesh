// Sources: the community-plugin switch with ADR-0012's disclosure, and every installed plugin with
// its badge, enabling, account and sign-in flows, settings from its JSON Schema, and uninstalling;
// plus installing a `.jfpkg` file.

import SwiftUI

struct SourcesSettings: View {
    @Environment(SourcesStore.self) private var sources
    @Environment(SettingsStore.self) private var settings
    @State private var disclosure: CommunityDisclosure?

    var body: some View {
        Form {
            if sources.unavailable {
                Section {
                    SourcesUnavailableNotice()
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

            Section {
                if sources.plugins.isEmpty {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("No sources installed")
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
        }
        .formStyle(.grouped)
        .communityDisclosure($disclosure) { request in
            if let id = request.pluginID {
                sources.acknowledgeAndEnable(id)
            } else {
                settings.setAllowCommunityPlugins(true)
            }
        }
        .sheet(item: Binding(get: { sources.signIn }, set: { flow in
            if flow == nil { sources.endSignIn() }
        })) { _ in
            SignInSheet()
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

            LabeledContent("Version", value: plugin.version)
            LabeledContent("Author", value: plugin.author)
            HStack {
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
        .confirmationDialog(Text("Uninstall “\(plugin.name)”?"), isPresented: $confirmUninstall,
                            titleVisibility: .visible) {
            Button("Uninstall", role: .destructive) {
                sources.uninstall(plugin.id)
            }
        } message: {
            Text("Its sign-in and settings are removed too.")
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

/// One setting. Text is sent when Return is pressed; toggles, steppers and choices at once.
/// Secrets never show the stored value.
private struct SchemaFieldRow: View {
    var plugin: SourcePlugin
    var field: SchemaField

    @Environment(SourcesStore.self) private var sources
    @State private var text = ""
    @State private var number: Double = 0
    @State private var loaded = false

    var body: some View {
        let current = SchemaForm.current(settingsJSON: plugin.settingsJSON, field: field)
        Group {
            switch field.kind {
            case .text(let secret, _):
                if secret {
                    SecureField(field.title, text: $text)
                        .onSubmit { save(.string(text)) }
                } else {
                    TextField(field.title, text: $text)
                        .onSubmit { save(.string(text)) }
                }
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
    }

    private func save(_ value: JSONValue) {
        sources.saveSettings(plugin.id, json: SchemaForm.withValue(settingsJSON: plugin.settingsJSON, key: field.key,
                                                                   value: value))
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
