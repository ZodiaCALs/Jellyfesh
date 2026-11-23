// The network the Mac is on, for the core (ADR-0025 §3, ADR-0026 §1): online or not, and whether
// the connection is metered, from NWPathMonitor. The core decides what follows (Auto quality is
// High while metered; downloads wait while offline). A path that is expensive (a phone's hotspot)
// or constrained (Low Data Mode) counts as metered. Reported at start and on every change, on the
// main actor.

import Foundation
import Network

/// What the core hears about the network.
struct NetworkState: Equatable, Sendable {
    var online: Bool
    var metered: Bool

    /// The state for a path's status and its costs.
    static func from(satisfied: Bool, expensive: Bool, constrained: Bool) -> NetworkState {
        NetworkState(online: satisfied, metered: satisfied && (expensive || constrained))
    }
}

@MainActor
final class NetworkMonitor {
    private var monitor: NWPathMonitor?
    private var reported: NetworkState?

    /// Calls `changed` with the network's state now and whenever it changes.
    func start(_ changed: @escaping @MainActor @Sendable (NetworkState) -> Void) {
        guard monitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            let state = NetworkState.from(satisfied: path.status == .satisfied, expensive: path.isExpensive,
                                          constrained: path.isConstrained)
            Task { @MainActor in
                guard let self, self.reported != state else { return }
                self.reported = state
                changed(state)
            }
        }
        monitor.start(queue: DispatchQueue(label: "org.jellyfesh.network", qos: .utility))
        self.monitor = monitor
    }

    func stop() {
        monitor?.cancel()
        monitor = nil
    }
}
