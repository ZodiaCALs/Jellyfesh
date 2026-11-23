// AI Lyrics in Now Playing (docs/ui.md "AI Lyrics, Customize and Pro"): with no lyrics, Create
// Synced Lyrics and what it needs first (a model to download, an API key); under plain lyrics,
// Time These Lyrics; while the song's job runs, its stage with a progress bar and Cancel; a failed
// job with Try Again. Made lyrics say "Made by AI Lyrics · {engine}", and the More menu offers
// Make Again and Delete Made Lyrics (asked first). The core decides what is needed
// (`AiLyricsText.step`); this shows it and sends the person's choice.

import SwiftUI

/// The song's AI Lyrics step. `timing` offers Time These Lyrics (over plain lyrics); `jobsOnly`
/// shows only a running or failed job (over lyrics the song already has).
struct LyricsMakingActions: View {
    var timing = false
    var jobsOnly = false

    @Environment(AppModel.self) private var model
    @Environment(AiLyricsStore.self) private var aiLyrics
    @Environment(ProStore.self) private var pro
    @Environment(PlaybackStore.self) private var playback
    @Environment(\.palette) private var palette

    var body: some View {
        let step = AiLyricsText.step(job: aiLyrics.job(forEntry: playback.nowPlaying?.uid), state: aiLyrics.state)
        VStack(spacing: Tokens.Space.s2) {
            switch step {
            case .none:
                EmptyView()
            case .create:
                if !jobsOnly {
                    Button(timing ? LocalizedStringKey("Time These Lyrics") : LocalizedStringKey("Create Synced Lyrics")) {
                        aiLyrics.make(.current, pro: pro)
                    }
                }
            case .running(let job, let stage, let progress):
                note(stage)
                if let progress {
                    ProgressView(value: progress)
                        .frame(maxWidth: 220)
                } else {
                    ProgressView()
                        .controlSize(.small)
                }
                Button("Cancel") {
                    aiLyrics.cancel(job: job)
                }
            case .failed(let message):
                note(AiLyricsText.failed(message))
                Button("Try Again") {
                    aiLyrics.make(.current, pro: pro)
                }
            case .downloadModel(let id, let text):
                if !jobsOnly {
                    note(text)
                    Button("Download") {
                        aiLyrics.download(model: id)
                    }
                }
            case .downloadingModel(let id, let text, let fraction):
                if !jobsOnly {
                    note(text)
                    ProgressView(value: fraction)
                        .frame(maxWidth: 220)
                    Button("Cancel") {
                        aiLyrics.cancelDownload(model: id)
                    }
                }
            case .signIn:
                if !jobsOnly {
                    note(String(localized: "Add your API key to make lyrics online."))
                    Button("Open AI Lyrics Settings") {
                        model.extensionsFocus = aiLyrics.state.plugin
                        model.openSettings(.extensions)
                    }
                }
            case .unavailable(let reason):
                if !jobsOnly {
                    note(reason)
                }
            }
        }
        .multilineTextAlignment(.center)
    }

    private func note(_ text: String) -> some View {
        Text(text)
            .lookFont(size: 13)
            .foregroundStyle(palette[Tokens.Ink.ink70])
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// "Made by AI Lyrics · {engine}" under made lyrics.
struct MadeByLine: View {
    var engine: String
    @Environment(\.palette) private var palette

    var body: some View {
        Text(AiLyricsText.madeBy(engine: engine))
            .lookFont(size: 12)
            .foregroundStyle(palette[Tokens.Ink.ink50])
    }
}

/// Make Again and Delete Made Lyrics in Now Playing's More menu, for lyrics AI Lyrics made.
struct MadeLyricsMenuItems: View {
    /// Delete Made Lyrics asks first, outside the menu.
    @Binding var confirmDelete: Bool

    @Environment(AiLyricsStore.self) private var aiLyrics
    @Environment(LyricsStore.self) private var lyrics
    @Environment(PlaybackStore.self) private var playback
    @Environment(ProStore.self) private var pro

    var body: some View {
        if !playback.isRemote, lyrics.madeBy(for: playback.nowPlaying?.uid) != nil {
            Divider()
            if aiLyrics.isOffered {
                Button("Make Again") {
                    aiLyrics.make(.current, pro: pro)
                }
            }
            Button("Delete Made Lyrics") {
                confirmDelete = true
            }
        }
    }
}

extension View {
    /// "Delete These Lyrics?" before Delete Made Lyrics.
    func confirmDeletingMadeLyrics(_ isPresented: Binding<Bool>, aiLyrics: AiLyricsStore) -> some View {
        confirmationDialog(Text("Delete These Lyrics?"), isPresented: isPresented, titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                aiLyrics.deleteMadeLyrics(.current)
            }
        } message: {
            Text("You can make them again.")
        }
    }

    /// "Send This Song to {origin}?", before AI Lyrics first sends audio to an origin.
    func audioConsentQuestion(_ aiLyrics: AiLyricsStore) -> some View {
        alert(Text("Send This Song to \(aiLyrics.consentQuestion?.origin ?? "")?"),
              isPresented: Binding(get: { aiLyrics.consentQuestion != nil },
                                   set: { if !$0 { aiLyrics.consentQuestion = nil } }),
              presenting: aiLyrics.consentQuestion) { question in
            Button("Cancel", role: .cancel) {
                aiLyrics.consentQuestion = nil
            }
            Button("Allow") {
                aiLyrics.allow(question)
            }
        } message: { question in
            Text("AI Lyrics will send the song's audio to \(question.origin) to make its lyrics. The provider's terms and privacy policy apply, and it may charge your account.")
        }
    }
}
