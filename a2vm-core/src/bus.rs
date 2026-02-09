/// CPU communicates with all external devices through the Bus trait.
/// `read` takes `&mut self` because hardware reads can have side effects
/// (e.g., $C030 toggles the speaker, $C010 clears keyboard strobe).
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);

    fn set_cycle(&mut self, _cycle: u64) {}

    fn read_word(&mut self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi = self.read(addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    /// Read a 16-bit word with the NMOS 6502 page-wrap bug:
    /// if `addr` is $xxFF, the high byte is read from $xx00 instead of $(xx+1)00.
    fn read_word_page_wrap(&mut self, addr: u16) -> u16 {
        let lo = self.read(addr) as u16;
        let hi_addr = (addr & 0xFF00) | ((addr.wrapping_add(1)) & 0x00FF);
        let hi = self.read(hi_addr) as u16;
        (hi << 8) | lo
    }
}
