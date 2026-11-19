// The look (ADR-0020, docs/ui.md "The Look slice"): every value resolved by the core, so the app
// applies one thing. With `customized` false every value is the Luminous default, so v0.0.6 looks
// exactly as before. Views read the values through `\.look` (LookStyle in Design/Look.swift).

import Foundation
import Observation

@MainActor
@Observable
final class LookStore {
    private(set) var state: LookState = .luminous

    func apply(_ state: LookState) {
        self.state = state
    }
}
