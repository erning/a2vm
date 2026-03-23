// ── Shared vertex ───────────────────────────────────────────────────

struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) texCoord: vec2f,
};

@vertex
fn vertexMain(@builtin(vertex_index) vid: u32) -> VertexOut {
    var out: VertexOut;
    let pos = vec2f(f32((vid << 1u) & 2u), f32(vid & 2u));
    out.position = vec4f(pos * 2.0 - 1.0, 0.0, 1.0);
    out.texCoord = vec2f(pos.x, 1.0 - pos.y);
    return out;
}

// ── Passthrough ─────────────────────────────────────────────────────

@group(0) @binding(0) var texSampler: sampler;
@group(0) @binding(1) var texSource: texture_2d<f32>;

@fragment
fn passthroughFragment(in: VertexOut) -> @location(0) vec4f {
    return textureSampleLevel(texSource, texSampler, in.texCoord, 0.0);
}

// ── CRT uniforms ────────────────────────────────────────────────────

struct CRTUniforms {
    bloomThreshold: f32,
    bloomIntensity: f32,
    scanlineIntensity: f32,
    blurDirectionX: f32,
    blurDirectionY: f32,
    sourceHeight: f32,
    curvature: f32,
    vignetteStrength: f32,
};

@group(0) @binding(2) var<uniform> u: CRTUniforms;

// ── Bloom extraction ────────────────────────────────────────────────

@fragment
fn bloomExtractFragment(in: VertexOut) -> @location(0) vec4f {
    let color = textureSampleLevel(texSource, texSampler, in.texCoord, 0.0);
    let lum = dot(color.rgb, vec3f(0.299, 0.587, 0.114));
    if (lum > u.bloomThreshold) {
        return color;
    }
    return vec4f(0.0, 0.0, 0.0, 1.0);
}

// ── Gaussian blur (9-tap separable) ─────────────────────────────────

@fragment
fn blurFragment(in: VertexOut) -> @location(0) vec4f {
    let weights = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    let dir = vec2f(u.blurDirectionX, u.blurDirectionY);

    var result = textureSampleLevel(texSource, texSampler, in.texCoord, 0.0) * weights[0];

    for (var i = 1; i < 5; i++) {
        let offset = dir * f32(i);
        result += textureSampleLevel(texSource, texSampler, in.texCoord + offset, 0.0) * weights[i];
        result += textureSampleLevel(texSource, texSampler, in.texCoord - offset, 0.0) * weights[i];
    }

    return result;
}

// ── CRT composite ───────────────────────────────────────────────────

@group(0) @binding(3) var texBloom: texture_2d<f32>;

@fragment
fn crtCompositeFragment(in: VertexOut) -> @location(0) vec4f {
    // Barrel distortion
    let centered = in.texCoord - 0.5;
    let r2 = dot(centered, centered);
    let uv = centered * (1.0 + u.curvature * r2) + 0.5;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4f(0.0, 0.0, 0.0, 1.0);
    }

    let scene = textureSampleLevel(texSource, texSampler, uv, 0.0);
    let bloom = textureSampleLevel(texBloom, texSampler, uv, 0.0);

    // Scanlines
    let row = fract(uv.y * u.sourceHeight);
    let scanline = smoothstep(0.0, 0.15, row) * smoothstep(1.0, 0.85, row);
    let scanlineFactor = mix(1.0, scanline, u.scanlineIntensity);

    // CRT phosphor background
    let crtBackground = vec3f(0.15, 0.15, 0.18);

    var color = max(scene.rgb, crtBackground) * scanlineFactor
              + bloom.rgb * u.bloomIntensity;

    // Vignette
    let vignette = 1.0 - u.vignetteStrength * r2;
    color *= vignette;

    return vec4f(color, 1.0);
}
