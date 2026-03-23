import Metal
import MetalKit

/// Manages the Metal pipeline for rendering the Apple II 280x192 RGBA framebuffer.
final class MetalRenderer {
    let device: MTLDevice
    let commandQueue: MTLCommandQueue
    let pipelineState: MTLRenderPipelineState
    let texture: MTLTexture

    let width: Int
    let height: Int

    init(device: MTLDevice, width: Int, height: Int) {
        self.device = device
        self.width = width
        self.height = height

        commandQueue = device.makeCommandQueue()!

        // Load compiled shader library from app bundle
        let libraryURL = Bundle.main.url(forResource: "Shaders", withExtension: "metallib")!
        let library = try! device.makeLibrary(URL: libraryURL)

        let vertexFunc = library.makeFunction(name: "vertexShader")!
        let fragmentFunc = library.makeFunction(name: "fragmentShader")!

        let desc = MTLRenderPipelineDescriptor()
        desc.vertexFunction = vertexFunc
        desc.fragmentFunction = fragmentFunc
        desc.colorAttachments[0].pixelFormat = .bgra8Unorm

        pipelineState = try! device.makeRenderPipelineState(descriptor: desc)

        // Create texture for the Apple II display
        let texDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        texDesc.usage = [.shaderRead]
        texture = device.makeTexture(descriptor: texDesc)!
    }

    /// Upload RGBA data to the GPU texture.
    func updateTexture(data: UnsafePointer<UInt8>) {
        texture.replace(
            region: MTLRegionMake2D(0, 0, width, height),
            mipmapLevel: 0,
            withBytes: data,
            bytesPerRow: width * 4
        )
    }

    /// Draw the texture as a fullscreen quad.
    func draw(in view: MTKView) {
        guard let drawable = view.currentDrawable,
              let passDesc = view.currentRenderPassDescriptor,
              let cmdBuf = commandQueue.makeCommandBuffer(),
              let encoder = cmdBuf.makeRenderCommandEncoder(descriptor: passDesc)
        else { return }

        encoder.setRenderPipelineState(pipelineState)
        encoder.setFragmentTexture(texture, index: 0)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
        encoder.endEncoding()

        cmdBuf.present(drawable)
        cmdBuf.commit()
    }
}
