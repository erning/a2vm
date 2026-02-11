# A2VM - Milestones

## Completed

### M1 - 6502 CPU Functional Validation - COMPLETE

- CPU passes Klaus Dormann functional test
- NMOS traps (BCD behavior, JMP indirect wrap, BRK/RTI/RTS semantics) are modeled

### M2 - Apple II Machine Integration - COMPLETE

- `AppleII` bus/memory integration
- ROM load and reset-vector boot
- keyboard latch/strobe and key injection path

### M3 - Applesoft/Monitor Execution Path - COMPLETE

- ROM boot flow reaches Monitor/BASIC paths
- frontend keyboard mapping supports interactive input

### M4 - Video Pipeline (TEXT/GR/HGR) - COMPLETE

- core bitmap renderer for text/graphics modes
- GUI RGBA pipeline with color/mono variants
- TUI Braille conversion display

### M5 - Disk II Read Path + Fast-Disk Trap - COMPLETE

- `.dsk` loading and nibblized track access
- slot ROM mapping and Disk II I/O switches
- fast-disk RWTS read trap

### M6 - Speaker Audio Output - COMPLETE

- `$C030` edge timeline capture
- PCM generation from cycle timestamps
- optional rodio playback in both frontends

## In Progress

### M7 - Code Quality and Maintainability

Current focus:

- CPU disassembler helper (`cpu/disasm.rs`)
- instruction-level CPU unit tests
- typed core error model for ROM/disk APIs
- Disk II write-path support through RWTS and write-mode hooks
- robustness for integration tests when external ROM/disk assets are unavailable

## Next Candidates

- broader unofficial-opcode compatibility beyond debug logging
- cycle-accurate disk timing model
- 80-column/extended video hardware support
- debugger overlays for TUI/GUI
- snapshot/save-state support
