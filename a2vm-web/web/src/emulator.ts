import init, { AppleIIWeb } from '../pkg/a2vm_web.js';
import { AudioHandler } from './audio.js';

const FLASH_HALF_PERIOD_MS = 267;
const NATIVE_WIDTH = 280;
const NATIVE_HEIGHT = 192;
const SCALE_STEP = 0.25;
const MIN_SCALE = 1;
const MAX_SCALE = 5;

export class Emulator {
  private wasm: AppleIIWeb | null = null;
  private wasmMemory: WebAssembly.Memory | null = null;
  private ctx: CanvasRenderingContext2D;
  private imageData: ImageData;
  private lastTime = 0;
  private flashOn = false;
  private animationId: number | null = null;
  private scale = 1;
  private container: HTMLElement;
  private audio: AudioHandler | null = null;

  constructor(
    private canvas: HTMLCanvasElement,
    private statusEl: HTMLDivElement,
    private diskStatusEl: HTMLSpanElement
  ) {
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Failed to get canvas context');
    this.ctx = ctx;
    this.imageData = ctx.createImageData(NATIVE_WIDTH, NATIVE_HEIGHT);
    this.container = canvas.parentElement || document.body;
    
    this.setupCanvas();
    this.setupResizeHandler();
  }

  private setupCanvas() {
    this.canvas.style.width = `${NATIVE_WIDTH}px`;
    this.canvas.style.height = `${NATIVE_HEIGHT}px`;
  }

  private setupResizeHandler() {
    window.addEventListener('resize', () => {
      this.fitToContainer();
    });
  }

  async init() {
    const wasmModule = await init();
    this.wasmMemory = wasmModule.memory;
    this.wasm = new AppleIIWeb();
    this.wasm.reset();
    
    if (this.wasm && this.wasmMemory) {
      this.audio = new AudioHandler(this.wasm, this.wasmMemory);
      await this.audio.init();
    }
    
    this.fitToContainer();
    this.start();
  }

  private start() {
    this.lastTime = performance.now();
    const loop = (timestamp: number) => {
      this.tick(timestamp);
      this.animationId = requestAnimationFrame(loop);
    };
    this.animationId = requestAnimationFrame(loop);
  }

  private tick(timestamp: number) {
    if (!this.wasm || !this.wasmMemory) return;

    const deltaMs = timestamp - this.lastTime;
    this.lastTime = timestamp;

    this.wasm.tick(deltaMs);
    
    this.flashOn = Math.floor(timestamp / FLASH_HALF_PERIOD_MS) % 2 === 0;
    this.wasm.render(this.flashOn, 0);

    const ptr = this.wasm.frame_buffer_ptr();
    const data = new Uint8ClampedArray(this.wasmMemory.buffer, ptr, NATIVE_WIDTH * NATIVE_HEIGHT * 4);
    this.imageData.data.set(data);
    
    this.ctx.putImageData(this.imageData, 0, 0);

    this.updateStatus();
  }

  private updateStatus() {
    if (!this.wasm) return;
    
    const modeNames = ['TEXT', 'GR', 'HGR'];
    const mode = modeNames[this.wasm.display_mode()] || '???';
    const diskStatus = this.wasm.is_motor_on() 
      ? `D:T${this.wasm.disk_track()}` 
      : 'D:--';
    const turboStatus = this.wasm.is_turbo() ? ` TURBO:x${this.wasm.turbo_speed()}` : '';
    const fastStatus = this.wasm.is_fast_disk() ? ' FAST' : '';
    const audioStatus = this.audio?.isEnabled() ? ' AUDIO' : '';
    
    this.statusEl.textContent = 
      `PC:${this.wasm.pc().toString(16).toUpperCase().padStart(4, '0')} ` +
      `A:${this.wasm.a().toString(16).toUpperCase().padStart(2, '0')} ` +
      `X:${this.wasm.x().toString(16).toUpperCase().padStart(2, '0')} ` +
      `Y:${this.wasm.y().toString(16).toUpperCase().padStart(2, '0')} ` +
      `SP:${this.wasm.sp().toString(16).toUpperCase().padStart(2, '0')} ` +
      `${mode} ${diskStatus}${turboStatus}${fastStatus}${audioStatus}`;
    
    const hasD1 = this.wasm.has_disk(0);
    const hasD2 = this.wasm.has_disk(1);
    if (hasD1 || hasD2) {
      this.diskStatusEl.textContent = 
        `D1:${hasD1 ? '✓' : '✗'} D2:${hasD2 ? '✓' : '✗'}`;
    } else {
      this.diskStatusEl.textContent = 'No disks loaded';
    }
  }

  async toggleAudio(): Promise<boolean> {
    if (!this.audio) return false;
    return this.audio.toggle();
  }

  scaleUp() {
    this.setScale(this.scale + SCALE_STEP);
  }

  scaleDown() {
    this.setScale(this.scale - SCALE_STEP);
  }

  fitToContainer() {
    const containerWidth = this.container.clientWidth - 40;
    const containerHeight = window.innerHeight * 0.6;
    
    const scaleX = containerWidth / NATIVE_WIDTH;
    const scaleY = containerHeight / NATIVE_HEIGHT;
    const newScale = Math.min(scaleX, scaleY, MAX_SCALE);
    
    this.setScale(Math.max(newScale, MIN_SCALE));
  }

  private setScale(newScale: number) {
    this.scale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, newScale));
    
    this.canvas.style.width = `${NATIVE_WIDTH * this.scale}px`;
    this.canvas.style.height = `${NATIVE_HEIGHT * this.scale}px`;
    
    this.updateScaleDisplay();
  }

  private updateScaleDisplay() {
    const scaleEl = document.getElementById('scale-value');
    if (scaleEl) {
      scaleEl.textContent = `${Math.round(this.scale * 100)}%`;
    }
  }

  getScale(): number {
    return this.scale;
  }

  async loadRom(file: File) {
    if (!this.wasm) return;
    
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      this.wasm.load_rom(data);
      this.wasm.reset();
      console.log('ROM loaded successfully');
    } catch (e) {
      console.error('Failed to load ROM:', e);
      alert('Failed to load ROM: ' + e);
    }
  }

  async loadDisk(file: File, drive: number) {
    if (!this.wasm) return;
    
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      this.wasm.load_disk(data, drive, false);
      console.log(`Disk loaded into drive ${drive + 1}`);
    } catch (e) {
      console.error('Failed to load disk:', e);
      alert('Failed to load disk: ' + e);
    }
  }

  exportDisk(drive: number) {
    if (!this.wasm) return;
    
    const data = this.wasm.export_disk(drive);
    if (data) {
      const blob = new Blob([data], { type: 'application/octet-stream' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `disk${drive + 1}.dsk`;
      a.click();
      URL.revokeObjectURL(url);
    } else {
      alert('No disk in drive ' + (drive + 1));
    }
  }

  reset() {
    if (!this.wasm) return;
    this.wasm.reset();
  }

  toggleTurbo() {
    if (!this.wasm) return;
    const isTurbo = this.wasm.toggle_turbo();
    console.log('Turbo:', isTurbo ? 'ON' : 'OFF');
  }

  toggleFastDisk() {
    if (!this.wasm) return;
    const isFast = !this.wasm.is_fast_disk();
    this.wasm.set_fast_disk(isFast);
    console.log('Fast disk:', isFast ? 'ON' : 'OFF');
  }

  handleKeyDown(e: KeyboardEvent) {
    if (!this.wasm) return;
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    
    const ascii = this.mapKey(e);
    if (ascii !== null) {
      this.wasm.key_press(ascii);
      e.preventDefault();
    }
  }

  handleKeyUp(_e: KeyboardEvent) {
    if (!this.wasm) return;
    this.wasm.clear_key_strobe();
  }

  private mapKey(e: KeyboardEvent): number | null {
    if (e.key.length === 1) {
      const code = e.key.charCodeAt(0);
      if (code < 128) return code;
    }
    
    switch (e.key) {
      case 'Enter': return 0x0D;
      case 'Backspace': return 0x7F;
      case 'Delete': return 0x7F;
      case 'Escape': return 0x1B;
      case 'Tab': return 0x09;
      case 'ArrowLeft': return 0x08;
      case 'ArrowRight': return 0x15;
      case 'ArrowUp': return 0x0B;
      case 'ArrowDown': return 0x0A;
      case ' ': return 0x20;
      default: return null;
    }
  }

  destroy() {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
    }
    this.audio?.destroy();
  }
}
