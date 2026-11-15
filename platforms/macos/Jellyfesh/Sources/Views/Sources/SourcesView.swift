// Sources (docs/ui.md): what sources are, Add a Source (the core's services, connected as in
// Settings), Your Sources (the installed plugins with their badges and accounts; choosing one
// browses its catalogue page by page) and Sources Settings…, where installing, enabling and each
// source's settings live.

import SwiftUI

struct SourcesView: View {
    @Environment(AppModel.self) private var model
    @Environment(SourcesStore.self) private var sources
    @Environment(\.openSettings) private var openSettings
    @Environment(\.palette) private var palette
    /// The service whose Connect sheet is open.
    @State private var connecting: SourcePreset?

    var body: some View {
        List {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                PageHeader(title: String(localized: "Sources"), subtitle: nil)
                Text("Sources bring music from servers and services into Jellyfesh.")
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.vertical, Tokens.Space.s4)
            .trackListRow()
            if sources.unavailable {
                SourcesUnavailableNotice()
                    .padding(.bottom, Tokens.Space.s3)
                    .trackListRow()
            }
            if !sources.presets.isEmpty {
                SectionTitle("Add a Source")
                    .padding(.top, Tokens.Space.s3)
                    .trackListRow()
                ForEach(sources.presets) { preset in
                    PresetRow(preset: preset, plugin: sources.plugin(preset.plugin), iconSize: 40) {
                        connecting = preset
                    }
                    .typography(Tokens.TypeRoles.listTitle)
                    .frame(minHeight: Tokens.Player.rowHeight)
                    .trackListRow()
                }
            }
            SectionTitle("Your Sources")
                .padding(.top, Tokens.Space.s5)
                .trackListRow()
            if sources.plugins.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    Text("No Sources Installed")
                        .typography(Tokens.TypeRoles.listTitle)
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                    Text("Install a source plugin from a .jfpkg file.")
                        .typography(Tokens.TypeRoles.listSecondary)
                        .foregroundStyle(palette[Tokens.Ink.ink70])
                }
                .padding(.vertical, Tokens.Space.s2)
                .accessibilityElement(children: .combine)
                .trackListRow()
            }
            ForEach(sources.plugins) { plugin in
                if plugin.enabled && plugin.canBrowse {
                    NavigationLink(value: Route.browse(plugin: plugin.id, node: nil, title: plugin.name)) {
                        SourceRow(plugin: plugin)
                    }
                    .trackListRow()
                } else {
                    SourceRow(plugin: plugin)
                        .trackListRow()
                }
            }
            HStack {
                Button("Sources Settings…") {
                    model.showSettings(.sources, using: openSettings)
                }
                .buttonStyle(.bordered)
                Spacer(minLength: 0)
            }
            .padding(.vertical, Tokens.Space.s5)
            .trackListRow()
        }
        .fieldList()
        .navigationTitle(Text("Sources"))
        .connectSourceFlow($connecting, in: .main)
    }
}

/// A plugin with its badge line and account.
struct SourceRow: View {
    var plugin: SourcePlugin
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            PluginIcon(plugin: plugin, size: 40)
            VStack(alignment: .leading, spacing: 1) {
                Text(plugin.name)
                    .typography(Tokens.TypeRoles.listTitle)
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                Text(CountText.joined([SourceText.badge(plugin), SourceText.account(plugin)]))
                    .typography(Tokens.TypeRoles.listSecondary)
                    .foregroundStyle(palette[Tokens.Ink.ink70])
            }
            Spacer(minLength: 0)
            if !plugin.enabled {
                Text("Off")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(palette[Tokens.Ink.ink50])
            }
        }
        .frame(height: Tokens.Player.rowHeight)
        .accessibilityElement(children: .combine)
    }
}

/// The plugin's own icon when it has one, else a generic glyph (also when there is no plugin, for
/// a service in Add a Source whose plugin is not listed).
struct PluginIcon: View {
    var plugin: SourcePlugin?
    var size: CGFloat

    var body: some View {
        if let path = plugin?.iconPath {
            ArtworkView(source: .file(path), size: size, cornerRadius: Tokens.Radius.rMd,
                        placeholder: "antenna.radiowaves.left.and.right")
        } else {
            ArtworkView(source: nil, size: size, cornerRadius: Tokens.Radius.rMd,
                        placeholder: "antenna.radiowaves.left.and.right")
        }
    }
}

/// The badge and account lines of a source (ADR-0004, ADR-0012), and of a service to add.
enum SourceText {
    static func badge(_ plugin: SourcePlugin) -> String {
        var parts = [plugin.badge == .official ? String(localized: "Official") : String(localized: "Community · unofficial")]
        if plugin.vendorPlayback {
            parts.append(String(localized: "Vendor playback"))
        }
        return parts.joined(separator: " · ")
    }

    static func account(_ plugin: SourcePlugin) -> String? {
        if let account = plugin.account {
            return String(localized: "Signed in as \(account)")
        }
        return plugin.signInMethods.isEmpty ? nil : String(localized: "Not signed in")
    }

    /// The line under a service in Add a Source: its account once signed in, else what it is
    /// (by the service's id; one without a description has no line).
    static func detail(_ preset: SourcePreset) -> String? {
        if let account = preset.account {
            return String(localized: "Signed in as \(account)")
        }
        switch preset.id {
        case "jellyfin": return String(localized: "Your Jellyfin server")
        case "navidrome": return String(localized: "Your Navidrome server")
        case "subsonic": return String(localized: "Airsonic, Gonic and other Subsonic servers")
        default: return nil
        }
    }
}

/// Said once a source command answered that sources are not in this build of the core yet.
struct SourcesUnavailableNotice: View {
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s3) {
            Image(systemName: "clock.badge.checkmark")
                .font(.system(size: 18))
                .foregroundStyle(palette[Tokens.Semantic.info])
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                Text("Sources Are on Their Way")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(palette[Tokens.Ink.ink90])
                Text("This build of the player cannot install, sign in to or browse sources yet. They arrive in an update.")
                    .font(.system(size: 12))
                    .foregroundStyle(palette[Tokens.Ink.ink70])
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(Tokens.Space.s4)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glass(.mist, in: RoundedRectangle(cornerRadius: Tokens.Radius.rLg, style: .continuous))
        .accessibilityElement(children: .combine)
    }
}

/// One container of a source's catalogue (`node` nil is its root), page by page.
struct BrowseView: View {
    var plugin: String
    var node: String?
    var title: String

    @Environment(SourcesStore.self) private var sources
    @Environment(PlaybackStore.self) private var playback
    @Environment(Router.self) private var router
    @State private var model: BrowseModel?

    var body: some View {
        let items = model?.items ?? []
        List {
            PageHeader(title: title, subtitle: sources.plugin(plugin)?.name)
                .padding(.vertical, Tokens.Space.s4)
                .trackListRow()
            ForEach(items) { item in
                row(for: item)
                    .trackListRow()
            }
            if let model, model.nextPage != nil {
                HStack {
                    Spacer()
                    Button("Load More") { model.loadMore(sources) }
                        .disabled(model.isLoading)
                    Spacer()
                }
                .trackListRow()
            }
        }
        .fieldList()
        .overlay {
            if let model {
                if model.unavailable {
                    SourcesUnavailableNotice()
                        .frame(maxWidth: 520)
                        .padding()
                } else if let error = model.error, items.isEmpty {
                    EmptyState(title: "This Source Did Not Answer", symbol: "exclamationmark.triangle", error: error)
                } else if model.isLoading && items.isEmpty {
                    ProgressView()
                } else if items.isEmpty {
                    EmptyState(title: "Nothing Here", symbol: "tray")
                }
            }
        }
        .navigationTitle(Text(title))
        .task(id: "\(plugin)\u{1F}\(node ?? "")") {
            let browse = BrowseModel(plugin: plugin, node: node)
            model = browse
            browse.start(sources)
        }
    }

    @ViewBuilder private func row(for item: SourceResult) -> some View {
        if item.kind == .track {
            TrackRowView(item: TrackItem(result: item, artwork: item.artwork.flatMap { ArtworkSource(hash: nil, location: $0) }),
                         isCurrent: item.track != nil && item.track == playback.nowPlaying?.track,
                         isPlaying: playback.isPlaying)
                .onTapGesture(count: 2) { play(from: item) }
                .contextMenu {
                    TrackActionsMenu(items: [TrackItem(result: item, artwork: nil)])
                }
                .accessibilityAction { play(from: item) }
        } else {
            ResultRow(item: item, artwork: item.artwork.flatMap { ArtworkSource(hash: nil, location: $0) })
                .contextMenu {
                    Button("Play") {
                        playback.play(.source(plugin: item.source, id: item.itemID))
                    }
                    Button("Play Next") {
                        playback.enqueue(.source(plugin: item.source, id: item.itemID), next: true)
                    }
                    Button("Play Last") {
                        playback.enqueue(.source(plugin: item.source, id: item.itemID), next: false)
                    }
                }
        }
    }

    private func play(from item: SourceResult) {
        guard let queue = model?.tracks(startingAt: item) else { return }
        playback.play(tracks: queue.tracks, from: queue.start)
    }
}
