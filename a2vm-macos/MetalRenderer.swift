import Metal
import MetalKit

/// Upscale factor: each emulator pixel becomes a NxN block.
private let upscaleFactor = 4

/// CRT effect settings.
struct CRTSettings {
    var bloomEnabled: Bool = true
    var scanlinesEnabled: Bool = true
    var bloomThreshold: Float = 0.4
    var bloomIntensity: Float = 1.5
    var scanlineIntensity: Float = 0.9
    var curvature: Float = 0.01
    var vignetteStrength: Float = 1.2
}

/// Uniform buffer matching the Metal `CRTUniforms` struct.
/// Must be all floats — no float2/SIMD to avoid alignment mismatch.
struct CRTUniforms {
    var bloomThreshold: Float = 0.4
    var bloomIntensity: Float = 1.5
    var scanlineIntensity: Float = 0.9
    var blurDirectionX: Float = 0.0
    var blurDirectionY: Float = 0.0
    var sourceHeight: Float = 768.0
    var curvature: Float = 0.01
    var vignetteStrength: Float = 1.2
}

/// Multi-pass Metal renderer:
///   source (280×192) → upscale (1120×768) → bloom → CRT composite → drawable
final class MetalRenderer {
    let device: MTLDevice
    let commandQueue: MTLCommandQueue

    // Source texture (280×192 RGBA from emulator)
    let sourceTexture: MTLTexture
    let width: Int
    let height: Int

    // Upscaled intermediate (1120×768)
    let upscaledTexture: MTLTexture
    let upscaledWidth: Int
    let upscaledHeight: Int

    // Pipeline states
    let passthroughPipeline: MTLRenderPipelineState
    let upscalePipeline: MTLRenderPipelineState
    let bloomExtractPipeline: MTLRenderPipelineState
    let blurPipeline: MTLRenderPipelineState
    let crtCompositePipeline: MTLRenderPipelineState

    // Offscreen textures for bloom (half of upscaled resolution)
    private var bloomTexture: MTLTexture!
    private var bloomTempTexture: MTLTexture!
    private var bloomWidth: Int = 0
    private var bloomHeight: Int = 0

    var settings = CRTSettings()

    init(device: MTLDevice, width: Int, height: Int) {
        self.device = device
        self.width = width
        self.height = height
        self.upscaledWidth = width * upscaleFactor
        self.upscaledHeight = height * upscaleFactor

        commandQueue = device.makeCommandQueue()!

        // Load shader library
        let libraryURL = Bundle.main.url(forResource: "Shaders", withExtension: "metallib")!
        let library = try! device.makeLibrary(URL: libraryURL)

        let vertexFunc = library.makeFunction(name: "vertexShader")!

        func makePipeline(_ fragName: String, pixelFormat: MTLPixelFormat) -> MTLRenderPipelineState {
            let desc = MTLRenderPipelineDescriptor()
            desc.vertexFunction = vertexFunc
            desc.fragmentFunction = library.makeFunction(name: fragName)!
            desc.colorAttachments[0].pixelFormat = pixelFormat
            return try! device.makeRenderPipelineState(descriptor: desc)
        }

        passthroughPipeline = makePipeline("fragmentShader", pixelFormat: .bgra8Unorm)
        upscalePipeline = makePipeline("fragmentShader", pixelFormat: .rgba8Unorm)
        bloomExtractPipeline = makePipeline("bloomExtractFragment", pixelFormat: .rgba16Float)
        blurPipeline = makePipeline("blurFragment", pixelFormat: .rgba16Float)
        crtCompositePipeline = makePipeline("crtCompositeFragment", pixelFormat: .bgra8Unorm)

        // Source texture (280×192)
        let srcDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba8Unorm,
            width: width, height: height, mipmapped: false
        )
        srcDesc.usage = [.shaderRead]
        sourceTexture = device.makeTexture(descriptor: srcDesc)!

        // Upscaled texture (1120×768)
        let upDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba8Unorm,
            width: upscaledWidth, height: upscaledHeight, mipmapped: false
        )
        upDesc.usage = [.shaderRead, .renderTarget]
        upscaledTexture = device.makeTexture(descriptor: upDesc)!

        // Bloom textures (half of upscaled = 560×384)
        bloomWidth = upscaledWidth / 2
        bloomHeight = upscaledHeight / 2
        let bloomDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba16Float,
            width: bloomWidth, height: bloomHeight, mipmapped: false
        )
        bloomDesc.usage = [.shaderRead, .renderTarget]
        bloomTexture = device.makeTexture(descriptor: bloomDesc)!
        bloomTempTexture = device.makeTexture(descriptor: bloomDesc)!
    }

    /// Upload RGBA data from the emulator.
    func updateTexture(data: UnsafePointer<UInt8>) {
        sourceTexture.replace(
            region: MTLRegionMake2D(0, 0, width, height),
            mipmapLevel: 0,
            withBytes: data,
            bytesPerRow: width * 4
        )
    }

    /// Main draw call.
    func draw(in view: MTKView) {
        guard let drawable = view.currentDrawable,
              let cmdBuf = commandQueue.makeCommandBuffer()
        else { return }

        let crtActive = settings.bloomEnabled || settings.scanlinesEnabled
            || settings.curvature > 0 || settings.vignetteStrength > 0

        if crtActive {
            drawCRT(cmdBuf: cmdBuf, view: view)
        } else {
            drawPassthrough(cmdBuf: cmdBuf, view: view)
        }

        cmdBuf.present(drawable)
        cmdBuf.commit()
    }

    // MARK: - Passthrough (no effects)

    private func drawPassthrough(cmdBuf: MTLCommandBuffer, view: MTKView) {
        guard let passDesc = view.currentRenderPassDescriptor,
              let encoder = cmdBuf.makeRenderCommandEncoder(descriptor: passDesc)
        else { return }

        encoder.setRenderPipelineState(passthroughPipeline)
        encoder.setFragmentTexture(sourceTexture, index: 0)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()
    }

    // MARK: - CRT multi-pass

    private func drawCRT(cmdBuf: MTLCommandBuffer, view: MTKView) {
        // Pass 0: Upscale source (280×192) → upscaled (1120×768) with nearest-neighbor
        renderPass(cmdBuf: cmdBuf, target: upscaledTexture,
                   pipeline: upscalePipeline, texture0: sourceTexture)

        // Pass 1: Bloom extraction (upscaled → bloomTempTexture)
        if settings.bloomEnabled {
            var uniforms = makeUniforms()
            renderPass(cmdBuf: cmdBuf, target: bloomTempTexture,
                       pipeline: bloomExtractPipeline, texture0: upscaledTexture,
                       uniforms: &uniforms)

            // Pass 2a: Horizontal blur
            blurPass(cmdBuf: cmdBuf, source: bloomTempTexture, dest: bloomTexture,
                     dirX: 1.0 / Float(bloomWidth), dirY: 0.0)

            // Pass 2b: Vertical blur
            blurPass(cmdBuf: cmdBuf, source: bloomTexture, dest: bloomTempTexture,
                     dirX: 0.0, dirY: 1.0 / Float(bloomHeight))
        }

        // Pass 3: CRT composite → drawable
        guard let passDesc = view.currentRenderPassDescriptor,
              let encoder = cmdBuf.makeRenderCommandEncoder(descriptor: passDesc)
        else { return }

        encoder.setRenderPipelineState(crtCompositePipeline)
        encoder.setFragmentTexture(upscaledTexture, index: 0)
        encoder.setFragmentTexture(
            settings.bloomEnabled ? bloomTempTexture : upscaledTexture, index: 1)

        var uniforms = makeUniforms()
        if !settings.bloomEnabled { uniforms.bloomIntensity = 0.0 }
        if !settings.scanlinesEnabled { uniforms.scanlineIntensity = 0.0 }
        encoder.setFragmentBytes(&uniforms, length: MemoryLayout<CRTUniforms>.size, index: 0)

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()
    }

    /// Generic offscreen render pass (fullscreen quad).
    private func renderPass(cmdBuf: MTLCommandBuffer, target: MTLTexture,
                             pipeline: MTLRenderPipelineState, texture0: MTLTexture,
                             uniforms: UnsafeMutablePointer<CRTUniforms>? = nil) {
        let passDesc = MTLRenderPassDescriptor()
        passDesc.colorAttachments[0].texture = target
        passDesc.colorAttachments[0].loadAction = .dontCare
        passDesc.colorAttachments[0].storeAction = .store

        guard let encoder = cmdBuf.makeRenderCommandEncoder(descriptor: passDesc)
        else { return }

        encoder.setRenderPipelineState(pipeline)
        encoder.setFragmentTexture(texture0, index: 0)
        if let u = uniforms {
            encoder.setFragmentBytes(u, length: MemoryLayout<CRTUniforms>.size, index: 0)
        }
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()
    }

    private func blurPass(cmdBuf: MTLCommandBuffer,
                           source: MTLTexture, dest: MTLTexture,
                           dirX: Float, dirY: Float) {
        let passDesc = MTLRenderPassDescriptor()
        passDesc.colorAttachments[0].texture = dest
        passDesc.colorAttachments[0].loadAction = .dontCare
        passDesc.colorAttachments[0].storeAction = .store

        guard let encoder = cmdBuf.makeRenderCommandEncoder(descriptor: passDesc)
        else { return }

        encoder.setRenderPipelineState(blurPipeline)
        encoder.setFragmentTexture(source, index: 0)

        var uniforms = makeUniforms()
        uniforms.blurDirectionX = dirX
        uniforms.blurDirectionY = dirY
        encoder.setFragmentBytes(&uniforms, length: MemoryLayout<CRTUniforms>.size, index: 0)

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()
    }

    private func makeUniforms() -> CRTUniforms {
        CRTUniforms(
            bloomThreshold: settings.bloomThreshold,
            bloomIntensity: settings.bloomIntensity,
            scanlineIntensity: settings.scanlineIntensity,
            blurDirectionX: 0.0,
            blurDirectionY: 0.0,
            sourceHeight: Float(height),  // 192 — one scanline per emulator row
            curvature: settings.curvature,
            vignetteStrength: settings.vignetteStrength
        )
    }
}
