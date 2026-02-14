import { Emulator } from './emulator.js';

async function main() {
  const canvas = document.getElementById('screen') as HTMLCanvasElement;
  const statusEl = document.getElementById('status') as HTMLDivElement;
  const diskStatusEl = document.getElementById('disk-status') as HTMLSpanElement;
  
  const emulator = new Emulator(canvas, statusEl, diskStatusEl);
  
  await emulator.init();
  
  document.getElementById('btn-reset')?.addEventListener('click', () => {
    emulator.reset();
  });
  
  document.getElementById('btn-turbo')?.addEventListener('click', () => {
    emulator.toggleTurbo();
  });
  
  document.getElementById('btn-fastdisk')?.addEventListener('click', () => {
    emulator.toggleFastDisk();
  });
  
  document.getElementById('btn-export')?.addEventListener('click', () => {
    emulator.exportDisk(0);
  });
  
  document.getElementById('btn-audio')?.addEventListener('click', () => {
    emulator.toggleAudio();
  });
  
  document.getElementById('btn-scale-up')?.addEventListener('click', () => {
    emulator.scaleUp();
  });
  
  document.getElementById('btn-scale-down')?.addEventListener('click', () => {
    emulator.scaleDown();
  });
  
  document.getElementById('btn-scale-fit')?.addEventListener('click', () => {
    emulator.fitToContainer();
  });
  
  document.getElementById('file-rom')?.addEventListener('change', (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) emulator.loadRom(file);
  });
  
  document.getElementById('file-disk1')?.addEventListener('change', (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) emulator.loadDisk(file, 0);
  });
  
  document.getElementById('file-disk2')?.addEventListener('change', (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) emulator.loadDisk(file, 1);
  });
  
  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey) {
      if (e.key === 'r') {
        e.preventDefault();
        emulator.reset();
      } else if (e.key === 't') {
        e.preventDefault();
        emulator.toggleTurbo();
      }
    }
    emulator.handleKeyDown(e);
  });
  
  window.addEventListener('keyup', (e) => {
    emulator.handleKeyUp(e);
  });
}

main().catch(console.error);
