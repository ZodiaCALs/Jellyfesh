// Now Playing's Comments pane (docs/ui.md "Comments"), for sources whose plugin shows comments:
// "Comments from {Service}" at the top, then "Popular" (the hot comments) and "Latest", each with
// the author, picture, time, likes and text; more load at the end of the list. Read-only. Empty:
// "No Comments Yet".

import SwiftUI

struct CommentsPanel: View {
    @Environment(AppModel.self) private var app
    @Environment(PlaybackStore.self) private var playback
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette
    @State private var model: CommentsModel?

    var body: some View {
        let now = playback.nowPlaying
        if let item = CommentsModel.item(for: now?.track, remote: playback.isRemote,
                                         showsComments: { sources.plugin($0)?.hasComments == true }),
           case .source(let plugin, _, _) = item {
            list(service: sources.name(of: plugin))
                .task(id: item) {
                    let comments = CommentsModel(item: item)
                    model = comments
                    comments.load(app.answers)
                }
        } else {
            EmptyState(title: "No Comments Yet", symbol: "text.bubble")
        }
    }

    private func list(service: String) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Tokens.Space.s3) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Comments from \(service)")
                        .lookFont(size: 12, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityAddTraits(.isHeader)
                    if let attribution = model?.attribution?.nilIfEmpty {
                        Text(attribution)
                            .lookFont(size: 11)
                            .foregroundStyle(palette[Tokens.Ink.ink50])
                    }
                }
                if let model {
                    if !model.popular.isEmpty {
                        heading("Popular")
                        ForEach(model.popular) { comment in
                            CommentRow(comment: comment)
                        }
                    }
                    if !model.latest.isEmpty {
                        heading("Latest")
                        ForEach(model.latest) { comment in
                            CommentRow(comment: comment)
                                .onAppear {
                                    if comment.id == model.latest.last?.id {
                                        model.loadMore(app.answers)
                                    }
                                }
                        }
                    }
                    if model.loading {
                        ProgressView()
                            .controlSize(.small)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, Tokens.Space.s3)
                    } else if let error = model.error {
                        VStack(spacing: Tokens.Space.s2) {
                            Text(error)
                                .lookFont(size: 13)
                                .foregroundStyle(palette[Tokens.Ink.ink70])
                                .multilineTextAlignment(.center)
                            Button("Try Again") {
                                if model.loaded {
                                    model.loadMore(app.answers)
                                } else {
                                    model.load(app.answers)
                                }
                            }
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, Tokens.Space.s3)
                    } else if model.loaded && model.popular.isEmpty && model.latest.isEmpty {
                        EmptyState(title: "No Comments Yet", symbol: "text.bubble")
                            .frame(minHeight: 200)
                    }
                }
            }
            .padding(.vertical, Tokens.Space.s4)
        }
        .scrollIndicators(.automatic)
        .accessibilityLabel(Text("Comments"))
    }

    private func heading(_ title: LocalizedStringKey) -> some View {
        Text(title)
            .lookFont(size: 13, weight: .semibold)
            .foregroundStyle(palette[Tokens.Ink.ink90])
            .padding(.top, Tokens.Space.s2)
            .accessibilityAddTraits(.isHeader)
    }
}

/// One comment: the author's picture and name, when it was written, its likes and its text.
private struct CommentRow: View {
    var comment: SourceComment
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s3) {
            ArtworkView(source: comment.avatar.flatMap { ArtworkSource(hash: nil, location: $0) }, size: 32,
                        cornerRadius: 16, placeholder: "person.crop.circle")
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                HStack(spacing: Tokens.Space.s2) {
                    Text(comment.author)
                        .lookFont(size: 13, weight: .semibold)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(1)
                    if let when = CommentText.when(comment.timeMs) {
                        Text(when)
                            .lookFont(size: 11)
                            .foregroundStyle(palette[Tokens.Ink.ink50])
                    }
                    Spacer(minLength: 0)
                    if let likes = comment.likes, likes > 0 {
                        Label {
                            Text(Int(clamping: likes), format: .number.notation(.compactName))
                        } icon: {
                            Image(systemName: "heart")
                        }
                        .labelStyle(.titleAndIcon)
                        .lookFont(size: 11)
                        .foregroundStyle(palette[Tokens.Ink.ink50])
                        .accessibilityLabel(Text("\(Int(clamping: likes)) likes"))
                    }
                }
                Text(comment.text)
                    .lookFont(size: 13)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
        }
        .padding(.vertical, Tokens.Space.s1)
        .accessibilityElement(children: .combine)
    }
}

/// A comment's time, as words.
enum CommentText {
    /// "3 days ago"; nil without a time.
    static func when(_ timeMs: Int64?) -> String? {
        guard let timeMs, timeMs > 0 else { return nil }
        let date = Date(timeIntervalSince1970: Double(timeMs) / 1000)
        return date.formatted(.relative(presentation: .named, unitsStyle: .abbreviated))
    }
}
