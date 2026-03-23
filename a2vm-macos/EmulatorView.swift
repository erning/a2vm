import AppKit
import MetalKit

/// MTKViewDelegate that drives the emulation loop and renders the display.
final class EmulatorView: NSView, MTKViewDelegate {
    private let emulator: EmulatorController
    private let renderer: MetalRenderer
    private let mtkView: MTKView
    private var rgbaBuffer: [UInt8]
    private var framePhase: UInt64 = 0

    init(frame: NSRect, emulator: EmulatorController) {
        self.emulator = emulator

        // Set up Metal
        let device = MTLCreateSystemDefaultDevice()!
        mtkView = MTKView(frame: frame, device: device)
        mtkView.colorPixelFormat = .bgra8Unorm
        mtkView.isPaused = false
        mtkView.enableSetNeedsDisplay = false
        mtkView.preferredFramesPerSecond = 60

        renderer = MetalRenderer(
            device: device,
            width: emulator.displayWidth,
            height: emulator.displayHeight
        )

        rgbaBuffer = [UInt8](repeating: 0, count: emulator.rgbaSize)

        super.init(frame: frame)

        mtkView.delegate = self
        mtkView.frame = bounds
        mtkView.autoresizingMask = [.width, .height]
        addSubview(mtkView)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) not implemented")
    }

    // MARK: - MTKViewDelegate

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        // No action needed — Metal handles scaling
    }

    func draw(in view: MTKView) {
        // Run emulation
        emulator.tick()

        // Always re-render: flash/cursor blink is time-based, not memory-triggered.
        // The 280×192 buffer is tiny — no performance concern.
        rgbaBuffer.withUnsafeMutableBufferPointer { ptr in
            emulator.renderRGBA(into: ptr.baseAddress!, framePhase: framePhase)
        }
        framePhase &+= 1

        rgbaBuffer.withUnsafeBufferPointer { ptr in
            renderer.updateTexture(data: ptr.baseAddress!)
        }

        renderer.draw(in: view)
    }

    // MARK: - Keyboard input

    override var acceptsFirstResponder: Bool { true }

    override func keyDown(with event: NSEvent) {
        if let ascii = mapKeyToAppleII(event) {
            emulator.keyPress(ascii)
        }
    }

    // Suppress beep for unhandled keys
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.command) {
            return super.performKeyEquivalent(with: event)
        }
        return true
    }
}
