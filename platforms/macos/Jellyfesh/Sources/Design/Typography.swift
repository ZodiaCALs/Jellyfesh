// Type roles and motion from the generated tokens (design/tokens.json). Platform faces only (Misty
// TYP-01): SF Pro, with PingFang and Hiragino as the system's CJK fallbacks.

import SwiftUI

extension Font.Weight {
    /// A CSS weight (100 to 900) as the nearest SwiftUI weight.
    init(cssWeight: Int) {
        switch cssWeight {
        case ..<150: self = .ultraLight
        case ..<250: self = .thin
        case ..<350: self = .light
        case ..<450: self = .regular
        case ..<550: self = .medium
        case ..<650: self = .semibold
        case ..<750: self = .bold
        case ..<850: self = .heavy
        default: self = .black
        }
    }
}

extension Font {
    /// A token type role (`Tokens.TypeRoles.listTitle`).
    static func token(_ role: Typography, size: CGFloat? = nil) -> Font {
        .system(size: size ?? role.size, weight: Font.Weight(cssWeight: role.weight))
    }
}

extension View {
    /// A type role with its line height: SwiftUI spaces lines at about 1.2 × the size, so the rest
    /// of the token's line height goes between lines.
    func typography(_ role: Typography, size: CGFloat? = nil, lineHeight: Double? = nil) -> some View {
        let points = size ?? role.size
        let extra = points * CGFloat((lineHeight ?? role.lineHeight) - 1.2)
        return font(.token(role, size: points)).lineSpacing(max(0, extra))
    }
}

/// Misty's motion tokens as SwiftUI animations. Pass `nil` under Reduce Motion: motion completes
/// instantly (MOT-12).
enum Motion {
    static func curve(_ c: CubicBezier, milliseconds: Int) -> Animation {
        .timingCurve(c.x1, c.y1, c.x2, c.y2, duration: Double(milliseconds) / 1000)
    }

    /// Entering and moving.
    static func fluid(_ milliseconds: Int) -> Animation { curve(Tokens.Motion.easeFluid, milliseconds: milliseconds) }
    /// Small objects only: presses, the play button.
    static func spring(_ milliseconds: Int) -> Animation { curve(Tokens.Motion.easeSpring, milliseconds: milliseconds) }
    /// Leaving.
    static func exit(_ milliseconds: Int) -> Animation { curve(Tokens.Motion.easeExit, milliseconds: milliseconds) }

    static var press: Animation { spring(Tokens.Motion.dur1Ms) }
    static var hover: Animation { fluid(Tokens.Motion.dur1Ms) }
    static var move: Animation { fluid(Tokens.Motion.dur2Ms) }
    /// The active lyric line scrolls into place (`lyrics.scroll`: dur-3, Fluid).
    static var lyricScroll: Animation { fluid(Tokens.Motion.dur3Ms) }
    /// Theme hue changes and the backdrop crossfade (`backdrop.crossfade`: dur-4, Fluid).
    static var themeChange: Animation { fluid(Tokens.Motion.dur4Ms) }
    /// `player.open-now-playing`: Misty's Deeper, 320 ms.
    static var openNowPlaying: Animation { fluid(320) }
    /// `player.close-now-playing`: 300 ms on Exit.
    static var closeNowPlaying: Animation { exit(300) }
    /// What motion becomes under reduced motion (the system's or Reduce motion and transparency):
    /// a short fade (`dur-1`), with nothing moving or scaling.
    static var shortFade: Animation { fluid(Tokens.Motion.dur1Ms) }
    /// A page fades in and rises (`dur-2` Fluid).
    static var pageIn: Animation { fluid(Tokens.Motion.dur2Ms) }
    /// The artwork springs back to full size when playing resumes (`dur-3` Spring).
    static var artworkScale: Animation { spring(Tokens.Motion.dur3Ms) }

    /// `animation`, or none under Reduce Motion.
    static func unless(_ reduceMotion: Bool, _ animation: Animation) -> Animation? {
        reduceMotion ? nil : animation
    }
}

extension AnyTransition {
    /// Now Playing rises out of the mini player or the player bar at the bottom of the window and
    /// sinks back into it: a slide and fade, since the split view's columns are separate hosting
    /// views and the artwork cannot grow out of the mini player's.
    static var deeper: AnyTransition {
        .asymmetric(
            insertion: .opacity
                .combined(with: .offset(y: 40))
                .combined(with: .scale(scale: 0.96, anchor: .bottom)),
            removal: .opacity.combined(with: .offset(y: 24))
        )
    }
}

/// Pages fade in and rise 8 pt when opened (`dur-2` Fluid); a page you come back to fades in
/// (`dur-1`). Under reduced motion both are a short fade.
struct PageEntrance: ViewModifier {
    static let rise: CGFloat = 8

    @Environment(\.effects) private var effects
    @State private var visible = false
    @State private var risen = false

    func body(content: Content) -> some View {
        content
            .opacity(visible ? 1 : 0)
            .offset(y: risen || !effects.animates ? 0 : Self.rise)
            .onAppear {
                if risen || !effects.animates {
                    withAnimation(Motion.shortFade) {
                        visible = true
                        risen = true
                    }
                } else {
                    withAnimation(Motion.pageIn) {
                        visible = true
                        risen = true
                    }
                }
            }
            .onDisappear { visible = false }
    }
}

extension View {
    func pageEntrance() -> some View {
        modifier(PageEntrance())
    }
}

/// m:ss (or h:mm:ss) for positions and durations.
enum TimeText {
    static func from(microseconds: Int64) -> String {
        from(seconds: Int(max(0, microseconds) / 1_000_000))
    }

    static func from(milliseconds: UInt64) -> String {
        from(seconds: Int(milliseconds / 1_000))
    }

    static func from(seconds total: Int) -> String {
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        let seconds = total % 60
        if hours > 0 {
            return String(format: "%ld:%02ld:%02ld", hours, minutes, seconds)
        }
        return String(format: "%ld:%02ld", minutes, seconds)
    }

    /// The time left with a true minus sign ("−2:28"), in whole seconds, so that it and the
    /// elapsed time always add up to the duration.
    static func remaining(positionUs: Int64, durationUs: Int64) -> String {
        let left = max(0, Int(max(0, durationUs) / 1_000_000) - Int(max(0, positionUs) / 1_000_000))
        return "\u{2212}" + from(seconds: left)
    }

    /// A spoken duration for VoiceOver ("3 minutes, 52 seconds").
    static func spoken(microseconds: Int64) -> String {
        let formatter = DateComponentsFormatter()
        formatter.unitsStyle = .full
        formatter.allowedUnits = microseconds >= 3_600_000_000 ? [.hour, .minute, .second] : [.minute, .second]
        return formatter.string(from: TimeInterval(max(0, microseconds)) / 1_000_000) ?? from(microseconds: microseconds)
    }
}
