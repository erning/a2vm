import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow!

    func applicationDidFinishLaunching(_ notification: Notification) {
        let emulator = EmulatorController()

        let scale = 3
        let contentWidth = emulator.displayWidth * scale   // 840
        let contentHeight = emulator.displayHeight * scale  // 576
        let contentRect = NSRect(x: 0, y: 0,
                                  width: contentWidth, height: contentHeight)

        window = NSWindow(
            contentRect: contentRect,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "A2VM"
        window.minSize = NSSize(width: emulator.displayWidth,
                                 height: emulator.displayHeight)
        window.contentAspectRatio = NSSize(width: emulator.displayWidth,
                                            height: emulator.displayHeight)

        let emulatorView = EmulatorView(frame: contentRect, emulator: emulator)
        window.contentView = emulatorView
        window.makeFirstResponder(emulatorView)

        window.center()
        window.makeKeyAndOrderFront(nil)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

// ── Entry point ─────────────────────────────────────────────────────

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.activate(ignoringOtherApps: true)
app.run()
