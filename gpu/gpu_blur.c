#include "gpu_blur.h"
#include "../drivers.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

// External function from kernel.c
extern void set_w1_global(const char *key, const char *val);

// Embedded GLSL Shaders

// Vertex shader for fullscreen quad
static const char *quad_vertex_shader = 
    "#version 100\n"
    "attribute vec2 a_position;\n"
    "varying vec2 v_texcoord;\n"
    "void main() {\n"
    "    v_texcoord = a_position * 0.5 + 0.5;\n"
    "    gl_Position = vec4(a_position, 0.0, 1.0);\n"
    "}\n";

// Fragment shader: 2x2 downsample
static const char *downsample_fragment_shader = 
    "#version 100\n"
    "precision mediump float;\n"
    "uniform sampler2D u_texture;\n"
    "uniform vec2 u_texel_size;\n"
    "varying vec2 v_texcoord;\n"
    "void main() {\n"
    "    vec2 texel = u_texel_size;\n"
    "    vec4 c00 = texture2D(u_texture, v_texcoord);\n"
    "    vec4 c01 = texture2D(u_texture, v_texcoord + vec2(texel.x, 0.0));\n"
    "    vec4 c10 = texture2D(u_texture, v_texcoord + vec2(0.0, texel.y));\n"
    "    vec4 c11 = texture2D(u_texture, v_texcoord + texel);\n"
    "    gl_FragColor = (c00 + c01 + c10 + c11) * 0.25;\n"
    "    gl_FragColor.a = 1.0;\n"
    "}\n";

// Fragment shader: Horizontal box blur
static const char *hblur_fragment_shader = 
    "#version 100\n"
    "precision mediump float;\n"
    "uniform sampler2D u_texture;\n"
    "uniform vec2 u_direction;\n"
    "uniform float u_radius;\n"
    "varying vec2 v_texcoord;\n"
    "void main() {\n"
    "    vec2 texel_size = u_direction;\n"
    "    vec4 result = vec4(0.0);\n"
    "    float count = 0.0;\n"
    "    \n"
    "    for (float i = -9.0; i <= 9.0; i++) {\n"
    "        vec2 offset = texel_size * i;\n"
    "        result += texture2D(u_texture, v_texcoord + offset);\n"
    "        count += 1.0;\n"
    "    }\n"
    "    \n"
    "    gl_FragColor = result / count;\n"
    "    gl_FragColor.a = 1.0;\n"
    "}\n";

// Fragment shader: Vertical box blur
static const char *vblur_fragment_shader = 
    "#version 100\n"
    "precision mediump float;\n"
    "uniform sampler2D u_texture;\n"
    "uniform vec2 u_direction;\n"
    "uniform float u_radius;\n"
    "varying vec2 v_texcoord;\n"
    "void main() {\n"
    "    vec2 texel_size = u_direction;\n"
    "    vec4 result = vec4(0.0);\n"
    "    float count = 0.0;\n"
    "    \n"
    "    for (float i = -9.0; i <= 9.0; i++) {\n"
    "        vec2 offset = texel_size * i;\n"
    "        result += texture2D(u_texture, v_texcoord + offset);\n"
    "        count += 1.0;\n"
    "    }\n"
    "    \n"
    "    gl_FragColor = result / count;\n"
    "    gl_FragColor.a = 1.0;\n"
    "}\n";

// Fullscreen quad vertices (normalized device coordinates)
// Note: Currently unused as quad geometry is handled directly in draw calls
#ifdef UNUSED
static const float quad_vertices[] = {
    -1.0f, -1.0f,
     1.0f, -1.0f,
    -1.0f,  1.0f,
     1.0f,  1.0f,
};
#endif

int gpu_blur_init(gpu_blur_context_t *ctx, int width, int height, int radius) {
    if (!ctx || !g_gpu_available) return -1;
    
    ctx->width = width;
    ctx->height = height;
    ctx->radius = radius;
    
    // Create shader programs
    int ret = gpu_create_program(quad_vertex_shader, downsample_fragment_shader, 
                                  &ctx->downsample_program);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-DownsampleShader");
        return -1;
    }
    
    ret = gpu_create_program(quad_vertex_shader, hblur_fragment_shader,
                              &ctx->hblur_program);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-HBlurShader");
        return -1;
    }
    
    ret = gpu_create_program(quad_vertex_shader, vblur_fragment_shader,
                              &ctx->vblur_program);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-VBlurShader");
        return -1;
    }
    
    // Create ping-pong framebuffers
    int half_w = width / 2;
    int half_h = height / 2;
    
    ret = gpu_create_framebuffer(half_w, half_h, &ctx->blur_fb[0]);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-FB0");
        return -1;
    }
    
    ret = gpu_create_framebuffer(half_w, half_h, &ctx->blur_fb[1]);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-FB1");
        gpu_destroy_framebuffer(&ctx->blur_fb[0]);
        return -1;
    }
    
    // Create input/output textures
    ret = gpu_create_resource(width, height, &ctx->input_texture);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-InputTex");
        gpu_destroy_framebuffer(&ctx->blur_fb[0]);
        gpu_destroy_framebuffer(&ctx->blur_fb[1]);
        return -1;
    }
    
    ret = gpu_create_resource(half_w, half_h, &ctx->output_texture);
    if (ret != 0) {
        set_w1_global("--gpuBlur", "Failed-OutputTex");
        gpu_destroy_resource(&ctx->input_texture);
        gpu_destroy_framebuffer(&ctx->blur_fb[0]);
        gpu_destroy_framebuffer(&ctx->blur_fb[1]);
        return -1;
    }
    
    set_w1_global("--gpuBlur", "Initialized");
    return 0;
}

void gpu_blur_cleanup(gpu_blur_context_t *ctx) {
    if (!ctx) return;
    
    gpu_destroy_framebuffer(&ctx->blur_fb[0]);
    gpu_destroy_framebuffer(&ctx->blur_fb[1]);
    gpu_destroy_resource(&ctx->input_texture);
    gpu_destroy_resource(&ctx->output_texture);
}

int gpu_downsample_execute(gpu_blur_context_t *ctx,
                           const uint32_t *input_data,
                           uint32_t *out_buf,
                           int input_width, int input_height) {
    if (!ctx || !input_data || !out_buf) return -1;
    
    int half_w = input_width / 2;
    int half_h = input_height / 2;
    
    // Upload input texture
    int ret = gpu_upload_texture(&ctx->input_texture, input_data, 
                                  input_width * input_height * 4);
    if (ret != 0) return -1;
    
    // Use downsample shader
    gpu_use_program(&ctx->downsample_program);
    
    // Set uniforms
    float texel_w = 1.0f / (float)input_width;
    float texel_h = 1.0f / (float)input_height;
    gpu_set_uniform_float2(&ctx->downsample_program, "u_texel_size", texel_w, texel_h);
    
    // Bind output framebuffer
    gpu_bind_framebuffer(&ctx->blur_fb[0]);
    
    // Draw fullscreen quad
    gpu_clear_color(0.0f, 0.0f, 0.0f, 1.0f);
    gpu_draw_quad(-1.0f, -1.0f, 2.0f, 2.0f);
    
    gpu_flush();
    gpu_finish();
    
    // Download result
    ret = gpu_download_texture(&ctx->blur_fb[0].color_attachment, 
                                out_buf, half_w * half_h * 4);
    
    // Unbind framebuffer
    gpu_unbind_framebuffer();
    
    return ret;
}

int gpu_blur_execute(gpu_blur_context_t *ctx,
                     const uint32_t *input_data,
                     uint32_t *out_buf,
                     int input_width, int input_height) {
    if (!ctx || !input_data || !out_buf) return -1;
    
    int half_w = input_width / 2;
    int half_h = input_height / 2;
    
    // Step 1: Downsample
    int ret = gpu_downsample_execute(ctx, input_data, out_buf, 
                                      input_width, input_height);
    if (ret != 0) return -1;
    
    // Step 2: Upload downsampled result for blur
    ret = gpu_upload_texture(&ctx->input_texture, out_buf, half_w * half_h * 4);
    if (ret != 0) return -1;
    
    // Step 3: Horizontal blur
    gpu_use_program(&ctx->hblur_program);
    gpu_set_uniform_float2(&ctx->hblur_program, "u_direction", 1.0f / half_w, 0.0f);
    gpu_set_uniform_float(&ctx->hblur_program, "u_radius", (float)ctx->radius);
    
    gpu_bind_framebuffer(&ctx->blur_fb[1]);
    gpu_clear_color(0.0f, 0.0f, 0.0f, 1.0f);
    gpu_draw_quad(-1.0f, -1.0f, 2.0f, 2.0f);
    gpu_flush();
    gpu_finish();
    
    // Step 4: Vertical blur (read from blur_fb[1], write to blur_fb[0])
    gpu_use_program(&ctx->vblur_program);
    gpu_set_uniform_float2(&ctx->vblur_program, "u_direction", 0.0f, 1.0f / half_h);
    gpu_set_uniform_float(&ctx->vblur_program, "u_radius", (float)ctx->radius);
    
    gpu_bind_framebuffer(&ctx->blur_fb[0]);
    gpu_clear_color(0.0f, 0.0f, 0.0f, 1.0f);
    gpu_draw_quad(-1.0f, -1.0f, 2.0f, 2.0f);
    gpu_flush();
    gpu_finish();
    
    // Download final result
    ret = gpu_download_texture(&ctx->blur_fb[0].color_attachment, 
                                out_buf, half_w * half_h * 4);
    
    gpu_unbind_framebuffer();
    
    return ret;
}
