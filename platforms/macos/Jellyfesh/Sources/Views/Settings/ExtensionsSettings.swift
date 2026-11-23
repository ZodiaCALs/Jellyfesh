// Settings › Extensions (docs/ui.md "Settings"; ADR-0019, ADR-0020, ADR-0021): Jellyfesh Pro (where
// it comes from, Buy and Restore Purchases in the App Store build, Enter Licence Key… where keys
// work, Remove Licence… for a key, and the legal pages), then every extension (AI Lyrics, Customize
// and others: plugins that only make lyrics or set the look) with its switch, its account, its
// settings in the app's language under their headings, and its version. AI Lyrics adds its
// On-Device Models (Download, Cancel, Delete… asked first) and the origins it may send audio to
// (Stop Allowing). Customize and other appearance plugins add Reset to Defaults. A Pro extension
// says so while Pro is inactive. Install from File… is in the heading where the build may install
// plugins.

import SwiftUI

struct ExtensionsSettings: View {
    @Environment(AppModel.self) private var model
    @Environment(SourcesStore.self) private var sources
    /// The extensions shown open.
    @State private var expanded: Set<String> = []

    var body: some View {
        Form {
            ProSection()

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

/// Jellyfesh Pro: where it comes from, and what can be done about it here.
private struct ProSection: View {
    @Environment(ProStore.self) private var pro
    @Environment(\.palette) private var palette
    @State private var confirmRemove = false

    private let links = AboutInfo.links(website: AboutInfo.buildWebsite())

    var body: some View {
        Section {
            LabeledContent {
                if ProText.hasLicence(pro.state) {
                    Button("Remove Licence…") {
                        confirmRemove = true
                    }
                }
            } label: {
                Text(verbatim: "Jellyfesh Pro")
                Text(ProText.status(pro.state))
            }
            if !pro.isActive {
                if pro.sellsInAppStore {
                    HStack {
                        Button {
                            pro.buy()
                        } label: {
                            if let price = pro.price {
                                Text("Buy Jellyfesh Pro – \(price)")
                            } else {
                                ProgressView()
                                    .controlSize(.small)
                            }
                        }
                        .disabled(pro.buying || pro.price == nil)
                        Button("Restore Purchases") {
                            pro.restorePurchases()
                        }
                        .disabled(pro.buying)
                    }
                }
                if pro.state.licenceKeys {
                    Button("Enter Licence Key…") {
                        pro.startLicenceEntry(in: .main)
                    }
                }
                if let failure = pro.storeFailure {
                    Text(failure)
                        .font(.footnote)
                        .foregroundStyle(palette[Tokens.Semantic.danger])
                }
            }
            if let links {
                HStack(spacing: Tokens.Space.s4) {
                    Link("Licence Agreement", destination: links.licenceAgreement)
                    Link("Privacy Policy", destination: links.privacyPolicy)
                    Link("Terms of Sale", destination: links.termsOfSale)
                }
                .font(.footnote)
            }
        }
        .confirmationDialog(Text("Remove the Licence Key?"), isPresented: $confirmRemove, titleVisibility: .visible) {
            Button("Remove Licence", role: .destructive) {
                pro.removeLicence()
            }
        } message: {
            Text("Pro features stop until you enter a key or buy Jellyfesh Pro.")
        }
    }
}

/// One extension: its name and summary, and when open its switch, account, settings and version.
private struct ExtensionRow: View {
    var plugin: SourcePlugin
    @Binding var expanded: Bool

    @Environment(SourcesStore.self) private var sources
    @Environment(ProStore.self) private var pro
    @Environment(AiLyricsStore.self) private var aiLyrics

    var body: some View {
        let needsPro = plugin.requiresPro && !pro.isActive
        let makesLyrics = aiLyrics.state.plugin == plugin.id
        DisclosureGroup(isExpanded: $expanded) {
            Toggle("Enabled", isOn: Binding(get: { plugin.enabled }, set: { sources.setEnabled(plugin.id, $0) }))
            if needsPro {
                HStack {
                    Text("Part of Jellyfesh Pro.")
                        .foregroundStyle(.secondary)
                    Spacer()
                    Button("Learn More…") {
                        pro.require()
                    }
                }
            }
            if !plugin.signInMethods.isEmpty {
                LabeledContent("Account") {
                    account
                }
            }
            PluginSettingsForm(plugin: plugin)
            if makesLyrics, !aiLyrics.state.models.isEmpty {
                heading("On-Device Models")
                ForEach(aiLyrics.state.models) { model in
                    SpeechModelRow(model: model)
                }
            }
            if makesLyrics {
                let consents = aiLyrics.state.consents.filter { $0.plugin == plugin.id }
                if !consents.isEmpty {
                    heading("Sending Audio")
                    ForEach(consents, id: \.self) { consent in
                        LabeledContent(consent.origin) {
                            Button("Stop Allowing") {
                                aiLyrics.stopAllowing(consent)
                            }
                        }
                    }
                }
            }
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

    private func heading(_ title: LocalizedStringKey) -> some View {
        Text(title)
            .font(.subheadline.weight(.semibold))
            .foregroundStyle(.secondary)
            .padding(.top, Tokens.Space.s1)
            .accessibilityAddTraits(.isHeader)
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

/// An on-device speech model: "Name · Size" with Download, its download with Cancel, or Ready with
/// Delete… (asked first).
private struct SpeechModelRow: View {
    var model: SpeechModelInfo

    @Environment(AiLyricsStore.self) private var aiLyrics
    @State private var confirmDelete = false

    var body: some View {
        LabeledContent {
            switch model.progress {
            case .notDownloaded:
                Button("Download") {
                    aiLyrics.download(model: model.id)
                }
            case .downloading(let fraction):
                HStack {
                    Text("Downloading… \(AiLyricsText.percent(fraction))")
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                    Button("Cancel") {
                        aiLyrics.cancelDownload(model: model.id)
                    }
                }
            case .ready:
                HStack {
                    Text("Ready")
                        .foregroundStyle(.secondary)
                    Button("Delete…") {
                        confirmDelete = true
                    }
                }
            case .failed(let message):
                HStack {
                    Text(message)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                    Button("Download") {
                        aiLyrics.download(model: model.id)
                    }
                }
            }
        } label: {
            Text(AiLyricsText.modelLine(model))
        }
        .confirmationDialog(Text("Delete the \(model.name) Model?"), isPresented: $confirmDelete,
                            titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                aiLyrics.delete(model: model.id)
            }
        } message: {
            Text("You can download it again.")
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
