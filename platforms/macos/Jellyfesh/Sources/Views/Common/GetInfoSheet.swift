// Get Info (docs/ui.md "Details"): from the song menu and the album and artist More menus. A song
// shows About This Song and Audio; an album or an artist, its picture, notes or biography and
// facts. What the row knows shows at once; the core's details (`Details`) fill in the rest, with
// the source's credit when it asks for one.

import SwiftUI

extension View {
    /// Get Info's sheet, for the window whose menus offer it.
    func getInfoSheet() -> some View {
        modifier(GetInfoPresenter())
    }
}

private struct GetInfoPresenter: ViewModifier {
    @Environment(Router.self) private var router

    func body(content: Content) -> some View {
        content.sheet(item: Binding(get: { router.infoRequest }, set: { router.infoRequest = $0 })) { request in
            GetInfoSheet(request: request)
        }
    }
}

struct GetInfoSheet: View {
    var request: InfoRequest

    @Environment(AppModel.self) private var app
    @Environment(Router.self) private var router
    @Environment(\.palette) private var palette
    @State private var answer: DetailsAnswer?
    @State private var failure: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            HStack(spacing: Tokens.Space.s3) {
                if let image = answer?.image {
                    ArtworkView(source: ArtworkSource(hash: nil, location: image), size: 64,
                                cornerRadius: request.kind == .artist ? 32 : Tokens.Radius.rLg)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(heading)
                        .font(.headline)
                        .accessibilityAddTraits(.isHeader)
                    if let title = answer?.title.nilIfEmpty ?? request.known.title {
                        Text(title)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                    }
                }
            }
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                    switch request.kind {
                    case .song:
                        InfoSection(lines: DetailsRows.song(answer, known: request.known))
                        InfoSection(title: String(localized: "Audio"), lines: DetailsRows.audio(answer, known: request.known))
                    case .album, .artist:
                        if let notes = answer?.notes?.trimmingCharacters(in: .whitespacesAndNewlines), !notes.isEmpty {
                            ExpandableNotes(text: notes)
                        }
                        InfoSection(lines: DetailsRows.facts(answer))
                    }
                    if let attribution = answer?.attribution?.nilIfEmpty {
                        Text(attribution)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                    if let problem = answer?.error?.nilIfEmpty ?? failure {
                        Text(problem)
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    } else if answer == nil {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack {
                Spacer()
                Button("Done") {
                    router.infoRequest = nil
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(Tokens.Space.s5)
        .frame(width: 460, height: 520)
        .task(id: request.id) {
            app.answers.details(request.item, onAnswer: { answer = $0 }, onFailure: { failure = $0 })
        }
    }

    private var heading: String {
        switch request.kind {
        case .song: return String(localized: "About This Song")
        case .album: return String(localized: "About This Album")
        case .artist: return String(localized: "About This Artist")
        }
    }
}

/// Rows of Get Info: a label and its value, in a grouped surface.
struct InfoSection: View {
    var title: String? = nil
    var lines: [InfoLine]

    @Environment(\.palette) private var palette

    var body: some View {
        if !lines.isEmpty {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                if let title {
                    Text(title)
                        .lookFont(size: 12, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityAddTraits(.isHeader)
                }
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                        LabeledContent {
                            Text(line.value)
                                .foregroundStyle(palette[Tokens.Ink.ink90])
                                .textSelection(.enabled)
                                .multilineTextAlignment(.trailing)
                                .lineLimit(line.key == .file ? 4 : nil)
                        } label: {
                            Text(InfoWords.label(line.key))
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                        }
                        .lookFont(size: 13)
                    }
                }
                .padding(Tokens.Space.s4)
                .frame(maxWidth: .infinity, alignment: .leading)
                .surfaceCard()
            }
        }
    }
}
