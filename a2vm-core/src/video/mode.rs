/// Display mode state, controlled by soft switches $C050-$C057.
#[derive(Clone, Debug)]
pub struct DisplayMode {
    pub text: bool,  // $C051/$C050: TEXT on/off
    pub mixed: bool, // $C053/$C052: mixed mode (4 text rows at bottom)
    pub page2: bool, // $C055/$C054: display page 2
    pub hires: bool, // $C057/$C056: hi-res mode
}

/// Color mode for GUI display rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DisplayColorMode {
    /// Full color (Lo-Res 16-color, Hi-Res NTSC artifact colors).
    #[default]
    Color,
    /// Monochrome (green phosphor).
    Monochrome,
    /// Monochrome with simulated CRT scanlines.
    MonochromeScanlines,
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self {
            text: true,
            mixed: false,
            page2: false,
            hires: false,
        }
    }
}
