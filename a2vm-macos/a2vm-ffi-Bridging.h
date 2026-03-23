#ifndef A2VM_FFI_BRIDGING_H
#define A2VM_FFI_BRIDGING_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/// Opaque emulator handle.
typedef struct A2VMEmulator A2VMEmulator;

// Lifecycle
A2VMEmulator* _Nonnull a2vm_create(void);
void a2vm_destroy(A2VMEmulator* _Nonnull emu);

// Emulation
void a2vm_tick(A2VMEmulator* _Nonnull emu);
void a2vm_reset(A2VMEmulator* _Nonnull emu);

// Input
void a2vm_key_press(A2VMEmulator* _Nonnull emu, uint8_t ascii);

// Video
void a2vm_render_rgba(A2VMEmulator* _Nonnull emu,
                      uint8_t* _Nonnull buf,
                      uint8_t color_mode,
                      uint64_t frame_phase);
bool a2vm_video_dirty(const A2VMEmulator* _Nonnull emu);
uint32_t a2vm_display_width(void);
uint32_t a2vm_display_height(void);

#endif
