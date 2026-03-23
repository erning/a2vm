#include <metal_stdlib>
using namespace metal;

// ── Shared vertex ───────────────────────────────────────────────────

struct VertexOut {
    float4 position [[position]];
    float2 texCoord;
};

// Fullscreen triangle (no vertex buffer needed).
vertex VertexOut vertexShader(uint vid [[vertex_id]]) {
    VertexOut out;
    float2 pos = float2((vid << 1) & 2, vid & 2);
    out.position = float4(pos * 2.0 - 1.0, 0.0, 1.0);
    out.texCoord = float2(pos.x, 1.0 - pos.y);
    return out;
}

// ── Passthrough (clean display, no effects) ─────────────────────────

fragment float4 fragmentShader(VertexOut in [[stage_in]],
                                texture2d<float> tex [[texture(0)]]) {
    constexpr sampler s(mag_filter::nearest, min_filter::nearest);
    return tex.sample(s, in.texCoord);
}

// ── CRT uniforms ────────────────────────────────────────────────────

struct CRTUniforms {
    float bloomThreshold;
    float bloomIntensity;
    float scanlineIntensity;
    float blurDirectionX;
    float blurDirectionY;
    float sourceHeight;       // emulator pixel rows (192)
    float curvature;          // barrel distortion strength
    float vignetteStrength;   // edge darkening
};

// ── Bloom extraction ────────────────────────────────────────────────

fragment float4 bloomExtractFragment(VertexOut in [[stage_in]],
                                      texture2d<float> tex [[texture(0)]],
                                      constant CRTUniforms &u [[buffer(0)]]) {
    constexpr sampler s(mag_filter::nearest, min_filter::nearest);
    float4 color = tex.sample(s, in.texCoord);
    float lum = dot(color.rgb, float3(0.299, 0.587, 0.114));
    return (lum > u.bloomThreshold) ? color : float4(0.0, 0.0, 0.0, 1.0);
}

// ── Gaussian blur (9-tap separable) ─────────────────────────────────

fragment float4 blurFragment(VertexOut in [[stage_in]],
                              texture2d<float> tex [[texture(0)]],
                              constant CRTUniforms &u [[buffer(0)]]) {
    constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);

    const float weights[5] = { 0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216 };

    float4 result = tex.sample(s, in.texCoord) * weights[0];

    for (int i = 1; i < 5; i++) {
        float2 offset = float2(u.blurDirectionX, u.blurDirectionY) * float(i);
        result += tex.sample(s, in.texCoord + offset) * weights[i];
        result += tex.sample(s, in.texCoord - offset) * weights[i];
    }

    return result;
}

// ── CRT composite (scene + bloom + scanlines + CRT background) ─────

fragment float4 crtCompositeFragment(VertexOut in [[stage_in]],
                                      texture2d<float> sceneTex [[texture(0)]],
                                      texture2d<float> bloomTex [[texture(1)]],
                                      constant CRTUniforms &u [[buffer(0)]]) {
    constexpr sampler linear_s(mag_filter::linear, min_filter::linear,
                                address::clamp_to_edge);

    // Barrel distortion: remap UV from center
    float2 centered = in.texCoord - 0.5;
    float r2 = dot(centered, centered);
    float2 uv = centered * (1.0 + u.curvature * r2) + 0.5;

    // Discard pixels outside the curved screen area
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return float4(0.0, 0.0, 0.0, 1.0);
    }

    float4 scene = sceneTex.sample(linear_s, uv);
    float4 bloom = bloomTex.sample(linear_s, uv);

    // Scanlines: sharp dark gaps between emulator pixel rows
    float row = fract(uv.y * u.sourceHeight);
    float scanline = smoothstep(0.0, 0.15, row) * smoothstep(1.0, 0.85, row);
    float scanlineFactor = mix(1.0, scanline, u.scanlineIntensity);

    // CRT phosphor background: never fully black, warm dark gray like a real CRT
    float3 crtBackground = float3(0.15, 0.15, 0.18);

    float3 color = max(scene.rgb, crtBackground) * scanlineFactor
                 + bloom.rgb * u.bloomIntensity;

    // Vignette: darken edges/corners
    float vignette = 1.0 - u.vignetteStrength * r2;
    color *= vignette;

    return float4(color, 1.0);
}
