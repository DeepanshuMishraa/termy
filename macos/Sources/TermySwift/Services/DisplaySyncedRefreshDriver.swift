import CoreVideo
import Foundation

final class DisplaySyncedRefreshDriver: @unchecked Sendable {
    var cadence: RefreshCadence = .active

    private var displayLink: CVDisplayLink?
    private var fallbackTimer: Timer?
    private let onTick: @MainActor () -> Void
    private var lastDeliveredAt: Date?
    private var isRunning = false
    private var thermalState = ProcessInfo.processInfo.thermalState
    private var thermalObserver: NSObjectProtocol?

    init(onTick: @escaping @MainActor () -> Void) {
        self.onTick = onTick
        thermalObserver = NotificationCenter.default.addObserver(
            forName: ProcessInfo.thermalStateDidChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.thermalState = ProcessInfo.processInfo.thermalState
        }
    }

    deinit {
        if let thermalObserver {
            NotificationCenter.default.removeObserver(thermalObserver)
        }
    }

    /// The minimum gap between ticks, raised under thermal pressure so a hot
    /// machine throttles the 60 Hz active cadence rather than fighting the fans.
    private var effectiveInterval: TimeInterval {
        max(cadence.interval, Self.thermalFloor(thermalState))
    }

    static func thermalFloor(_ state: ProcessInfo.ThermalState) -> TimeInterval {
        switch state {
        case .nominal, .fair:
            return 0
        case .serious:
            return 1.0 / 30.0
        case .critical:
            return 1.0 / 15.0
        @unknown default:
            return 0
        }
    }

    func start(cadence: RefreshCadence) {
        self.cadence = cadence
        guard !isRunning else {
            return
        }
        isRunning = true
        lastDeliveredAt = nil

        var link: CVDisplayLink?
        let status = CVDisplayLinkCreateWithActiveCGDisplays(&link)
        guard status == kCVReturnSuccess, let link else {
            startFallbackTimer()
            return
        }

        displayLink = link
        let context = Unmanaged.passUnretained(self).toOpaque()
        CVDisplayLinkSetOutputCallback(link, displayLinkCallback, context)
        if CVDisplayLinkStart(link) != kCVReturnSuccess {
            displayLink = nil
            startFallbackTimer()
        }
    }

    func stop() {
        isRunning = false
        fallbackTimer?.invalidate()
        fallbackTimer = nil
        if let displayLink {
            CVDisplayLinkStop(displayLink)
            CVDisplayLinkSetOutputCallback(displayLink, nil, nil)
            self.displayLink = nil
        }
        lastDeliveredAt = nil
    }

    private func startFallbackTimer() {
        fallbackTimer?.invalidate()
        let timer = Timer(timeInterval: cadence.interval, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.deliverTickIfDue()
            }
        }
        timer.tolerance = cadence.interval * 0.2
        RunLoop.main.add(timer, forMode: .common)
        fallbackTimer = timer
    }

    fileprivate func displayLinkTick() {
        DispatchQueue.main.async { [weak self] in
            self?.deliverTickIfDue()
        }
    }

    @MainActor
    private func deliverTickIfDue() {
        guard isRunning else {
            return
        }

        let now = Date()
        if let lastDeliveredAt,
           now.timeIntervalSince(lastDeliveredAt) < effectiveInterval {
            return
        }

        lastDeliveredAt = now
        onTick()
    }
}

private let displayLinkCallback: CVDisplayLinkOutputCallback = {
    _, _, _, _, _, userInfo in
    guard let userInfo else {
        return kCVReturnSuccess
    }

    let driver = Unmanaged<DisplaySyncedRefreshDriver>
        .fromOpaque(userInfo)
        .takeUnretainedValue()
    driver.displayLinkTick()
    return kCVReturnSuccess
}
