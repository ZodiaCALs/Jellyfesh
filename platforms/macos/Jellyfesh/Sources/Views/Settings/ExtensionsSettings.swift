// Settings › Extensions (docs/ui.md "Settings"; ADR-0019, ADR-0020, ADR-0021): every extension
// (AI Lyrics, Customize and others: plugins that only make lyrics or set the look) with its switch,
// its account, its settings in the app's language under their headings, and its version. Customize
// and other appearance plugins add Reset to Defaults. A Pro extension says so while Pro is
// inactive. Install from File… is in the heading where the build may install plugins.

import SwiftUI

struct ExtensionsSettings: View {
    @Environment(AppModel.self) private var model
    @Environment(SourcesStore.self) private var sources
    /// The extensions shown open.
    @State private var expanded: Set<String> = []

    var body: some View {
        Form {
            Section {
                ForEach(sources.extensions) { plugin in
                    ExtensionRow(plugin: plugin, expanded: Binding(
                        get: { expanded.contains(plugin.id) },
                        set: { open in
                            if open { expanded.insert(plugin.id) } else { expanded.remove(plugin.id) }
                        }
                    ))
                }
            } header: {
                HStack {
                    Text("Extensions")
                    Spacer()
                    if sources.canInstallPlugins {
                        Button("Install from File…") {
                            FilePanels.choosePlugin { url in sources.install(path: url.path) }
                        }
                        .buttonStyle(.link)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .connectSourceFlow(.constant(nil), in: .extensions)
        .onAppear(perform: reveal)
        .onChange(of: model.extensionsFocus) { _, _ in
            reveal()
        }
    }

    /// Opens the extension asked for (Settings › Appearance › Customize…).
    private func reveal() {
        guard let id = model.extensionsFocus else { return }
        expanded.insert(id)
        model.extensionsFocus = nil
    }
}

/// One extension: its name and summary, and when open its switch, account, settings and version.
private struct ExtensionRow: View {
    var plugin: SourcePlugin
    @Binding var expanded: Bool

    @Environment(SourcesStore.self) private var sources
    @Environment(ProStore.self) private var pro

    var body: some View {
        let needsPro = plugin.requiresPro && !pro.isActive
        DisclosureGroup(isExpanded: $expanded) {
            Toggle("Enabled", isOn: Binding(get: { plugin.enabled }, set: { sources.setEnabled(plugin.id, $0) }))
            if needsPro {
                HStack {
                    Text("Part of Jellyfesh Pro.")
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button("Learn More…") {
                        pro.sheetShown = true
                    }
                }
            }
            if !plugin.signInMethods.isEmpty {
                LabeledContent("Account") {
                    account
                }
            }
            PluginSettingsForm(plugin: plugin)
            HStack {
                Text(CountText.joined([String(localized: "Version \(plugin.version)"), plugin.author]))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                Spacer()
                if plugin.capabilities.contains("appearance") {
                    Button("Reset to Defaults") {
                        sources.resetSettings(plugin.id)
                    }
                }
            }
        } label: {
            HStack(spacing: Tokens.Space.s3) {
                PluginIcon(plugin: plugin, size: 28)
                VStack(alignment: .leading, spacing: 1) {
                    Text(plugin.name)
                    if let summary = plugin.summary, !summary.isEmpty {
                        Text(summary)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                    }
                }
                Spacer()
                if needsPro {
                    ProBadge()
                }
            }
        }
        .id(plugin.id)
    }

    /// Sign In, or what is signed in and Sign Out. A key-only sign-in (AI Lyrics' online mode)
    /// says "API Key Added".
    @ViewBuilder private var account: some View {
        if let name = plugin.account {
            HStack {
                Text(plugin.signInMethods == [.apiKey] ? String(localized: "API Key Added")
                                                       : String(localized: "Signed in as \(name)"))
                    .foregroundStyle(.secondary)
                Button("Sign Out") {
                    sources.logout(plugin.id)
                }
            }
        } else if let method = plugin.signInMethods.first {
            Button("Sign In") {
                sources.beginSignIn(plugin, method: method, in: .extensions)
            }
            .disabled(!plugin.enabled)
        }
    }
}

/// "Pro", on what Jellyfesh Pro opens while it is inactive.
struct ProBadge: View {
    @Environment(\.palette) private var palette

    var body: some View {
        Text(verbatim: "Pro")
            .font(.system(size: 10, weight: .bold))
            .foregroundStyle(palette[Tokens.Accent.accentText])
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(Capsule().fill(palette[Tokens.Accent.accentSoft]))
            .accessibilityLabel(Text(verbatim: "Jellyfesh Pro"))
    }
}
