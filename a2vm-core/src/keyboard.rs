#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleKey {
    Printable(char),
    Control(char),
    Enter,
    Backspace,
    Delete,
    Space,
    Left,
    Right,
    Up,
    Down,
    Escape,
    Tab,
}

pub fn map_apple_key(key: AppleKey) -> Option<u8> {
    match key {
        AppleKey::Printable(ch) => map_printable(ch),
        AppleKey::Control(ch) => map_control(ch),
        AppleKey::Enter => Some(0x0D),
        AppleKey::Backspace => Some(0x08),
        AppleKey::Delete => Some(0x7F),
        AppleKey::Space => Some(0x20),
        AppleKey::Left => Some(0x08),
        AppleKey::Right => Some(0x15),
        AppleKey::Up => Some(0x0B),
        AppleKey::Down => Some(0x0A),
        AppleKey::Escape => Some(0x1B),
        AppleKey::Tab => Some(0x09),
    }
}

fn map_printable(ch: char) -> Option<u8> {
    if !ch.is_ascii() {
        return None;
    }
    let mut ascii = ch as u8;
    if ascii.is_ascii_lowercase() {
        ascii -= 0x20;
    }
    Some(ascii)
}

fn map_control(ch: char) -> Option<u8> {
    if !ch.is_ascii_alphabetic() {
        return None;
    }
    let ascii = ch.to_ascii_uppercase() as u8;
    let ctrl = ascii.wrapping_sub(b'@');
    if (1..=26).contains(&ctrl) {
        Some(ctrl)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{map_apple_key, AppleKey};

    #[test]
    fn printable_keys_map_to_ascii() {
        assert_eq!(map_apple_key(AppleKey::Printable('a')), Some(b'A'));
        assert_eq!(map_apple_key(AppleKey::Printable('A')), Some(b'A'));
        assert_eq!(map_apple_key(AppleKey::Printable(' ')), Some(0x20));
        assert_eq!(map_apple_key(AppleKey::Printable('1')), Some(b'1'));
    }

    #[test]
    fn non_ascii_printable_is_rejected() {
        assert_eq!(map_apple_key(AppleKey::Printable('é')), None);
    }

    #[test]
    fn control_keys_map_to_apple_codes() {
        assert_eq!(map_apple_key(AppleKey::Control('a')), Some(0x01));
        assert_eq!(map_apple_key(AppleKey::Control('z')), Some(0x1A));
        assert_eq!(map_apple_key(AppleKey::Control('A')), Some(0x01));
        assert_eq!(map_apple_key(AppleKey::Control('1')), None);
    }

    #[test]
    fn named_keys_map_to_expected_values() {
        assert_eq!(map_apple_key(AppleKey::Enter), Some(0x0D));
        assert_eq!(map_apple_key(AppleKey::Backspace), Some(0x08));
        assert_eq!(map_apple_key(AppleKey::Delete), Some(0x7F));
        assert_eq!(map_apple_key(AppleKey::Space), Some(0x20));
        assert_eq!(map_apple_key(AppleKey::Left), Some(0x08));
        assert_eq!(map_apple_key(AppleKey::Right), Some(0x15));
        assert_eq!(map_apple_key(AppleKey::Up), Some(0x0B));
        assert_eq!(map_apple_key(AppleKey::Down), Some(0x0A));
        assert_eq!(map_apple_key(AppleKey::Escape), Some(0x1B));
        assert_eq!(map_apple_key(AppleKey::Tab), Some(0x09));
    }
}
