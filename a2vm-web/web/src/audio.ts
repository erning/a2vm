import type { AppleIIWeb } from '../pkg/a2vm_web.js';

const SAMPLE_RATE = 44100;

export class AudioHandler {
  private ctx: AudioContext | null = null;
  private workletNode: AudioWorkletNode | null = null;
  private wasm: AppleIIWeb | null = null;
  private enabled = false;
  private lastCycleCount = 0n;

  constructor(wasm: AppleIIWeb, _wasmMemory: WebAssembly.Memory) {
    this.wasm = wasm;
  }

  async init(): Promise<void> {
    try {
      this.ctx = new AudioContext({ sampleRate: SAMPLE_RATE });
      
      const workletCode = `
        class A2VMAudioProcessor extends AudioWorkletProcessor {
          constructor() {
            super();
            this.buffer = new Float32Array(1024);
            this.bufferIndex = 0;
            this.bufferFill = 0;
            
            this.port.onmessage = (event) => {
              if (event.data.type === 'samples') {
                const samples = event.data.samples;
                for (let i = 0; i < samples.length && this.bufferFill < 1024; i++) {
                  this.buffer[this.bufferFill++] = samples[i];
                }
              }
            };
          }
          
          process(inputs, outputs) {
            const output = outputs[0][0];
            
            for (let i = 0; i < output.length; i++) {
              if (this.bufferIndex < this.bufferFill) {
                output[i] = this.buffer[this.bufferIndex++];
              } else {
                output[i] = 0;
              }
            }
            
            if (this.bufferIndex >= this.bufferFill) {
              this.bufferIndex = 0;
              this.bufferFill = 0;
            }
            
            this.port.postMessage({ type: 'need_samples' });
            return true;
          }
        }
        
        registerProcessor('a2vm-audio', A2VMAudioProcessor);
      `;
      
      const blob = new Blob([workletCode], { type: 'application/javascript' });
      const url = URL.createObjectURL(blob);
      
      await this.ctx.audioWorklet.addModule(url);
      
      this.workletNode = new AudioWorkletNode(this.ctx, 'a2vm-audio', {
        outputChannelCount: [1],
      });
      
      this.workletNode.port.onmessage = (event) => {
        if (event.data.type === 'need_samples') {
          this.generateSamples();
        }
      };
      
      this.workletNode.connect(this.ctx.destination);
      
      URL.revokeObjectURL(url);
      
      console.log('Audio initialized');
    } catch (_e) {
      console.error('Failed to initialize audio:', _e);
    }
  }

  private generateSamples(): void {
    if (!this.wasm || !this.enabled) return;
    
    const currentCycles = this.wasm.cycles();
    const deltaCycles = currentCycles - this.lastCycleCount;
    
    if (deltaCycles > 0n) {
      const samples = this.wasm.generate_audio(SAMPLE_RATE, deltaCycles);
      
      if (samples.length > 0 && this.workletNode) {
        this.workletNode.port.postMessage({
          type: 'samples',
          samples: samples,
        });
      }
    }
    
    this.lastCycleCount = currentCycles;
  }

  async enable(): Promise<void> {
    if (!this.ctx) {
      await this.init();
    }
    
    if (this.ctx?.state === 'suspended') {
      await this.ctx.resume();
    }
    
    this.enabled = true;
    this.lastCycleCount = this.wasm?.cycles() || 0n;
    console.log('Audio enabled');
  }

  disable(): void {
    this.enabled = false;
    if (this.ctx?.state === 'running') {
      this.ctx.suspend();
    }
    console.log('Audio disabled');
  }

  toggle(): boolean {
    if (this.enabled) {
      this.disable();
      return false;
    } else {
      this.enable();
      return true;
    }
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  destroy(): void {
    this.disable();
    this.workletNode?.disconnect();
    this.ctx?.close();
  }
}
