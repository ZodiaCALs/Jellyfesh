// The Pro sheet (docs/ui.md "AI Lyrics, Customize and Pro"): opened by any Pro action while Pro is
// inactive. What Pro holds, Buy Jellyfesh Pro – {price} and Restore Purchases where this build sells
// Pro in the App Store, Enter Licence Key… where keys work, Not Now, and the Licence Agreement and
// Privacy Policy where the build knows the website. Enter Licence Key asks for the key and shows
// why the core refused one. Both windows that offer Pro actions attach them (`proSheet(in:)`), and
// they show in the window that asked.

import SwiftUI

extension View {
    /// The Pro sheet and Enter Licence Key, for a window with Pro actions.
    func proSheet(in window: ProStore.SheetWindow) -> some View {
        modifier(ProSheetPresenter(window: window))
    }
}

private struct ProSheetPresenter: ViewModifier {
    var window: ProStore.SheetWindow
    @Environment(ProStore.self) private var pro

    func body(content: Content) -> some View {
        content
            .sheet(isPresented: Binding(get: { pro.sheetShown && pro.sheetWindow == window },
                                        set: { if !$0 { pro.sheetShown = false } })) {
                ProSheet()
            }
            .sheet(isPresented: Binding(get: { pro.licenceEntryShown && !pro.sheetShown && pro.sheetWindow == window },
                                        set: { if !$0 { pro.licenceEntryShown = false } })) {
                LicenceKeySheet()
            }
    }
}

struct ProSheet: View {
    @Environment(ProStore.self) private var pro
    @Environment(\.palette) private var palette

    private let links = AboutInfo.links(website: AboutInfo.buildWebsite())

    var body: some View {
        VStack(spacing: Tokens.Space.s4) {
            Image(systemName: "sparkles")
                .font(.system(size: 36, weight: .semibold))
                .foregroundStyle(palette[Tokens.Accent.accent])
                .accessibilityHidden(true)
            Text(verbatim: "Jellyfesh Pro")
                .font(.title2.weight(.bold))
                .accessibilityAddTraits(.isHeader)
            Text("AI Lyrics makes synced lyrics for any song. Customize makes Jellyfesh yours: corners, glass, type, lyrics and layout.")
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            VStack(spacing: Tokens.Space.s2) {
                if pro.sellsInAppStore {
                    Button {
                        pro.buy()
                    } label: {
                        if let price = pro.price {
                            Text("Buy Jellyfesh Pro – \(price)")
                                .frame(maxWidth: .infinity)
                        } else {
                            ProgressView()
                                .controlSize(.small)
                                .frame(maxWidth: .infinity)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .keyboardShortcut(.defaultAction)
                    .disabled(pro.buying || pro.price == nil)
                    Button("Restore Purchases") {
                        pro.restorePurchases()
                    }
                    .disabled(pro.buying)
                }
                if pro.state.licenceKeys {
                    Button("Enter Licence Key…") {
                        pro.startLicenceEntry()
                    }
                }
                if let failure = pro.storeFailure {
                    Text(failure)
                        .font(.footnote)
                        .foregroundStyle(palette[Tokens.Semantic.danger])
                        .multilineTextAlignment(.center)
                }
            }
            .frame(maxWidth: 300)
            if let links {
                HStack(spacing: Tokens.Space.s4) {
                    Link("Licence Agreement", destination: links.licenceAgreement)
                    Link("Privacy Policy", destination: links.privacyPolicy)
                }
                .font(.footnote)
            }
            Button("Not Now") {
                pro.sheetShown = false
            }
            .keyboardShortcut(.cancelAction)
        }
        .padding(Tokens.Space.s6)
        .frame(width: 420)
        // Over the Pro sheet; Pro turning on closes both.
        .sheet(isPresented: Binding(get: { pro.licenceEntryShown }, set: { pro.licenceEntryShown = $0 })) {
            LicenceKeySheet()
        }
    }
}

/// Enter Licence Key: the key ("JFP1-…"), Cancel and Activate, and why the core refused a key.
struct LicenceKeySheet: View {
    @Environment(ProStore.self) private var pro
    @Environment(\.palette) private var palette
    @State private var key = ""
    /// A key was sent from this sheet: the core's answer about it shows.
    @State private var sent = false

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s3) {
            Text("Enter Licence Key")
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
            TextField("Licence Key", text: $key, prompt: Text(verbatim: "JFP1-…"))
                .textFieldStyle(.roundedBorder)
                .font(.system(.body, design: .monospaced))
                .autocorrectionDisabled()
                .onSubmit(activate)
            if let problem = problem {
                Text(problem)
                    .font(.footnote)
                    .foregroundStyle(palette[Tokens.Semantic.danger])
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack {
                if pro.activating {
                    ProgressView()
                        .controlSize(.small)
                }
                Spacer()
                Button("Cancel", role: .cancel) {
                    pro.licenceEntryShown = false
                }
                .keyboardShortcut(.cancelAction)
                Button("Activate", action: activate)
                    .keyboardShortcut(.defaultAction)
                    .disabled(key.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || pro.activating)
            }
        }
        .padding(Tokens.Space.s5)
        .frame(width: 380)
    }

    /// Why the key sent did not work: the core's reason, or the command's failure.
    private var problem: String? {
        guard sent, !pro.activating else { return nil }
        if let refused = pro.state.licenceProblem { return ProText.problem(refused) }
        return pro.failure
    }

    private func activate() {
        guard !key.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        sent = true
        pro.activate(key: key)
    }
}
