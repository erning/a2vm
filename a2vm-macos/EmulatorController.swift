import Foundation

/// Swift wrapper around the C FFI emulator handle.
final class EmulatorController {
    private let emu: OpaquePointer

    let displayWidth: Int
    let displayHeight: Int
    let rgbaSize: Int

    init() {
        emu = a2vm_create()
        displayWidth = Int(a2vm_display_width())
        displayHeight = Int(a2vm_display_height())
        rgbaSize = displayWidth * displayHeight * 4
    }

    deinit {
        a2vm_destroy(emu)
    }

    func tick() {
        a2vm_tick(emu)
    }

    func reset() {
        a2vm_reset(emu)
    }

    func keyPress(_ ascii: UInt8) {
        a2vm_key_press(emu, ascii)
    }

    var videoDirty: Bool {
        a2vm_video_dirty(emu)
    }

    func renderRGBA(into buffer: UnsafeMutablePointer<UInt8>,
                    colorMode: UInt8 = 0,
                    framePhase: UInt64 = 0) {
        a2vm_render_rgba(emu, buffer, colorMode, framePhase)
    }
}
