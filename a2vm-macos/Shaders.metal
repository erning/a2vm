#include <metal_stdlib>
using namespace metal;

struct VertexOut {
    float4 position [[position]];
    float2 texCoord;
};

// Fullscreen triangle (no vertex buffer needed).
vertex VertexOut vertexShader(uint vid [[vertex_id]]) {
    VertexOut out;
    // Generate a fullscreen triangle from vertex ID (0, 1, 2)
    float2 pos = float2((vid << 1) & 2, vid & 2);
    out.position = float4(pos * 2.0 - 1.0, 0.0, 1.0);
    out.texCoord = float2(pos.x, 1.0 - pos.y);
    return out;
}

fragment float4 fragmentShader(VertexOut in [[stage_in]],
                                texture2d<float> tex [[texture(0)]]) {
    constexpr sampler s(mag_filter::nearest, min_filter::nearest);
    return tex.sample(s, in.texCoord);
}
