import AppKit

/// Map an NSEvent key press to Apple II 7-bit ASCII.
///
/// Returns nil if the key has no Apple II equivalent.
func mapKeyToAppleII(_ event: NSEvent) -> UInt8? {
    let flags = event.modifierFlags
    let ctrl = flags.contains(.control)

    // Ignore Cmd-modified keys (handled as menu shortcuts)
    if flags.contains(.command) {
        return nil
    }

    // Control + letter → 0x01-0x1A
    if ctrl {
        if let chars = event.charactersIgnoringModifiers,
           let ch = chars.unicodeScalars.first,
           ch.value >= 0x61 && ch.value <= 0x7A { // a-z
            return UInt8(ch.value - 0x60) // Ctrl-A = 0x01, etc.
        }
        return nil
    }

    // Named keys
    switch event.keyCode {
    case 36:  return 0x0D  // Return
    case 51:  return 0x08  // Backspace → Left
    case 53:  return 0x1B  // Escape
    case 48:  return 0x09  // Tab
    case 49:  return 0x20  // Space
    case 123: return 0x08  // Left arrow
    case 124: return 0x15  // Right arrow
    case 125: return 0x0A  // Down arrow
    case 126: return 0x0B  // Up arrow
    case 117: return 0x7F  // Delete (forward)
    default: break
    }

    // Printable characters
    if let chars = event.characters, let ch = chars.unicodeScalars.first {
        let v = ch.value
        if v >= 0x20 && v <= 0x7E {
            return UInt8(v)
        }
    }

    return nil
}
