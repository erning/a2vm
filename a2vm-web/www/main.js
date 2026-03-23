import init, { Emulator, wasm_memory } from '../pkg/a2vm_web.js';

const EMU_WIDTH = 280;
const EMU_HEIGHT = 192;
const UPSCALE = 4;
const UP_WIDTH = EMU_WIDTH * UPSCALE;   // 1120
const UP_HEIGHT = EMU_HEIGHT * UPSCALE; // 768
const CPU_HZ = 1023000; // 1.023 MHz

const CRT = {
    bloomThreshold: 0.4,
    bloomIntensity: 1.5,
    scanlineIntensity: 0.9,
    blurDirX: 0.0,
    blurDirY: 0.0,
    sourceHeight: EMU_HEIGHT,
    curvature: 0.01,
    vignetteStrength: 1.2,
};

async function main() {
    // Init wasm
    await init();
    const emu = new Emulator(performance.now());

    // Check WebGPU
    if (!navigator.gpu) {
        document.getElementById('error').style.display = 'block';
        document.getElementById('error').textContent = 'WebGPU not supported in this browser.';
        return;
    }

    const adapter = await navigator.gpu.requestAdapter();
    const device = await adapter.requestDevice();
    const canvas = document.getElementById('screen');
    const ctx = canvas.getContext('webgpu');
    const format = navigator.gpu.getPreferredCanvasFormat();

    const ASPECT = EMU_WIDTH / EMU_HEIGHT; // 280/192 ≈ 1.458

    function resizeCanvas() {
        const dpr = window.devicePixelRatio || 1;
        const maxW = window.innerWidth;
        const maxH = window.innerHeight;
        let w, h;
        if (maxW / maxH > ASPECT) {
            h = maxH;
            w = Math.floor(h * ASPECT);
        } else {
            w = maxW;
            h = Math.floor(w / ASPECT);
        }
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        canvas.width = Math.floor(w * dpr);
        canvas.height = Math.floor(h * dpr);
        ctx.configure({ device, format, alphaMode: 'opaque' });
    }

    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);

    // Load shaders
    const shaderCode = await (await fetch('shaders.wgsl')).text();
    const shaderModule = device.createShaderModule({ code: shaderCode });

    // ── Textures ────────────────────────────────────────────────

    const sourceTexture = device.createTexture({
        size: [EMU_WIDTH, EMU_HEIGHT],
        format: 'rgba8unorm',
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });

    const upscaledTexture = device.createTexture({
        size: [UP_WIDTH, UP_HEIGHT],
        format: 'rgba8unorm',
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });

    const bloomW = UP_WIDTH >> 1, bloomH = UP_HEIGHT >> 1;
    const bloomTexA = device.createTexture({
        size: [bloomW, bloomH],
        format: 'rgba16float',
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });
    const bloomTexB = device.createTexture({
        size: [bloomW, bloomH],
        format: 'rgba16float',
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });

    // ── Samplers ────────────────────────────────────────────────

    const nearestSampler = device.createSampler({ magFilter: 'nearest', minFilter: 'nearest' });
    const linearSampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear' });

    // ── Uniform buffers (one per pass variant) ─────────────────

    function makeUniformBuf(overrides = {}) {
        const buf = device.createBuffer({
            size: 32, // 8 floats × 4 bytes
            usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
        });
        const data = new Float32Array([
            overrides.bloomThreshold ?? CRT.bloomThreshold,
            overrides.bloomIntensity ?? CRT.bloomIntensity,
            overrides.scanlineIntensity ?? CRT.scanlineIntensity,
            overrides.blurDirX ?? CRT.blurDirX,
            overrides.blurDirY ?? CRT.blurDirY,
            overrides.sourceHeight ?? CRT.sourceHeight,
            overrides.curvature ?? CRT.curvature,
            overrides.vignetteStrength ?? CRT.vignetteStrength,
        ]);
        device.queue.writeBuffer(buf, 0, data);
        return buf;
    }

    const uniformGeneral = makeUniformBuf();
    const uniformBlurH = makeUniformBuf({ blurDirX: 1.0 / bloomW, blurDirY: 0.0 });
    const uniformBlurV = makeUniformBuf({ blurDirX: 0.0, blurDirY: 1.0 / bloomH });

    // ── Bind group layouts ──────────────────────────────────────

    // Layout for passes with 1 texture (passthrough, bloom extract, blur)
    const singleTexLayout = device.createBindGroupLayout({
        entries: [
            { binding: 0, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
            { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
            { binding: 2, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
        ],
    });

    // Layout for CRT composite (2 textures)
    const dualTexLayout = device.createBindGroupLayout({
        entries: [
            { binding: 0, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
            { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
            { binding: 2, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
            { binding: 3, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        ],
    });

    // ── Pipelines ───────────────────────────────────────────────

    function makePipeline(layout, fragEntry, targetFormat) {
        return device.createRenderPipeline({
            layout: device.createPipelineLayout({ bindGroupLayouts: [layout] }),
            vertex: { module: shaderModule, entryPoint: 'vertexMain' },
            fragment: {
                module: shaderModule,
                entryPoint: fragEntry,
                targets: [{ format: targetFormat }],
            },
            primitive: { topology: 'triangle-list' },
        });
    }

    const upscalePipeline = makePipeline(singleTexLayout, 'passthroughFragment', 'rgba8unorm');
    const bloomExtractPipeline = makePipeline(singleTexLayout, 'bloomExtractFragment', 'rgba16float');
    const blurPipeline = makePipeline(singleTexLayout, 'blurFragment', 'rgba16float');
    const compositePipeline = makePipeline(dualTexLayout, 'crtCompositeFragment', format);

    // ── Bind groups ─────────────────────────────────────────────

    function makeBindGroup1(layout, sampler, texture, ubuf) {
        return device.createBindGroup({
            layout,
            entries: [
                { binding: 0, resource: sampler },
                { binding: 1, resource: texture.createView() },
                { binding: 2, resource: { buffer: ubuf } },
            ],
        });
    }

    function makeBindGroup2(layout, sampler, tex1, tex2, ubuf) {
        return device.createBindGroup({
            layout,
            entries: [
                { binding: 0, resource: sampler },
                { binding: 1, resource: tex1.createView() },
                { binding: 2, resource: { buffer: ubuf } },
                { binding: 3, resource: tex2.createView() },
            ],
        });
    }

    const bgUpscale = makeBindGroup1(singleTexLayout, nearestSampler, sourceTexture, uniformGeneral);
    const bgBloomExtract = makeBindGroup1(singleTexLayout, nearestSampler, upscaledTexture, uniformGeneral);
    const bgBlurH = makeBindGroup1(singleTexLayout, linearSampler, bloomTexA, uniformBlurH);
    const bgBlurV = makeBindGroup1(singleTexLayout, linearSampler, bloomTexB, uniformBlurV);
    const bgComposite = makeBindGroup2(dualTexLayout, linearSampler, upscaledTexture, bloomTexA, uniformGeneral);

    // ── Keyboard ────────────────────────────────────────────────

    document.addEventListener('keydown', (e) => {
        const ascii = mapKey(e);
        if (ascii !== null) {
            emu.key_press(ascii);
            e.preventDefault();
        }
    });

    function mapKey(e) {
        if (e.metaKey) return null;
        if (e.ctrlKey) {
            const ch = e.key.toLowerCase();
            if (ch >= 'a' && ch <= 'z') return ch.charCodeAt(0) - 0x60;
            return null;
        }
        switch (e.key) {
            case 'Enter': return 0x0D;
            case 'Backspace': return 0x08;
            case 'Escape': return 0x1B;
            case 'Tab': return 0x09;
            case 'ArrowLeft': return 0x08;
            case 'ArrowRight': return 0x15;
            case 'ArrowUp': return 0x0B;
            case 'ArrowDown': return 0x0A;
            case 'Delete': return 0x7F;
        }
        if (e.key.length === 1) {
            const code = e.key.charCodeAt(0);
            if (code >= 0x20 && code <= 0x7E) return code;
        }
        return null;
    }

    // ── Render loop ─────────────────────────────────────────────

    let lastTime = performance.now();
    let cycleAccum = 0;

    function frame(timestamp) {
        const dt = Math.min(timestamp - lastTime, 100); // cap at 100ms
        lastTime = timestamp;

        // Run emulation
        cycleAccum += dt * (CPU_HZ / 1000);
        const cycles = Math.floor(cycleAccum);
        cycleAccum -= cycles;
        if (cycles > 0) emu.run_cycles(cycles);

        // Render RGBA
        const ptr = emu.render_rgba(timestamp);
        const mem = wasm_memory();
        const rgba = new Uint8Array(mem.buffer, ptr, EMU_WIDTH * EMU_HEIGHT * 4);
        device.queue.writeTexture(
            { texture: sourceTexture },
            rgba,
            { bytesPerRow: EMU_WIDTH * 4 },
            [EMU_WIDTH, EMU_HEIGHT],
        );

        // GPU passes
        const encoder = device.createCommandEncoder();

        // Pass 0: Upscale (nearest)
        renderPass(encoder, upscaledTexture, upscalePipeline, bgUpscale);

        // Pass 1: Bloom extract
        renderPass(encoder, bloomTexA, bloomExtractPipeline, bgBloomExtract);

        // Pass 2a: Blur horizontal
        renderPass(encoder, bloomTexB, blurPipeline, bgBlurH);

        // Pass 2b: Blur vertical
        renderPass(encoder, bloomTexA, blurPipeline, bgBlurV);

        // Pass 3: CRT composite → canvas
        const canvasView = ctx.getCurrentTexture().createView();
        const pass = encoder.beginRenderPass({
            colorAttachments: [{
                view: canvasView,
                loadOp: 'clear',
                storeOp: 'store',
                clearValue: { r: 0, g: 0, b: 0, a: 1 },
            }],
        });
        pass.setPipeline(compositePipeline);
        pass.setBindGroup(0, bgComposite);
        pass.draw(3);
        pass.end();

        device.queue.submit([encoder.finish()]);
        requestAnimationFrame(frame);
    }

    function renderPass(encoder, target, pipeline, bindGroup) {
        const pass = encoder.beginRenderPass({
            colorAttachments: [{
                view: target.createView(),
                loadOp: 'clear',
                storeOp: 'store',
                clearValue: { r: 0, g: 0, b: 0, a: 1 },
            }],
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, bindGroup);
        pass.draw(3);
        pass.end();
    }

    requestAnimationFrame(frame);
}

main().catch(e => {
    document.getElementById('error').style.display = 'block';
    document.getElementById('error').textContent = `Error: ${e.message}`;
    console.error(e);
});
