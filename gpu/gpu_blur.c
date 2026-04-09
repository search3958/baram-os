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

    // GPU downsample: 2x2 box average
    for (int y = 0; y < half_h; y++) {
        const uint32_t *src0 = &input_data[(y*2)*input_width];
        const uint32_t *src1 = &input_data[(y*2+1)*input_width];
        uint32_t *dst = &out_buf[y * half_w];
        for (int x = 0; x < half_w; x++) {
            uint32_t c00 = src0[x*2], c01 = src0[x*2+1];
            uint32_t c10 = src1[x*2], c11 = src1[x*2+1];
            uint32_t r = ((c00>>16&0xFF) + (c01>>16&0xFF) + (c10>>16&0xFF) + (c11>>16&0xFF)) >> 2;
            uint32_t g = ((c00>>8&0xFF) + (c01>>8&0xFF) + (c10>>8&0xFF) + (c11>>8&0xFF)) >> 2;
            uint32_t b = ((c00&0xFF) + (c01&0xFF) + (c10&0xFF) + (c11&0xFF)) >> 2;
            dst[x] = 0xFF000000 | (r << 16) | (g << 8) | b;
        }
    }

    return 0;
}

int gpu_blur_execute(gpu_blur_context_t *ctx,
                     const uint32_t *input_data,
                     uint32_t *out_buf,
                     int input_width, int input_height) {
    if (!ctx || !input_data || !out_buf) return -1;

    int half_w = input_width / 2;
    int half_h = input_height / 2;

    // Step 1: Downsample (GPU shader simulation)
    int ret = gpu_downsample_execute(ctx, input_data, out_buf,
                                      input_width, input_height);
    if (ret != 0) return -1;

    // Step 2: Allocate temp buffer for blur passes
    uint32_t *blur_tmp = (uint32_t *)malloc(half_w * half_h * 4);
    if (!blur_tmp) return -1;

    // Step 3: Horizontal blur pass (GPU fragment shader simulation)
    int radius = ctx->radius;
    for (int y = 0; y < half_h; y++) {
        const uint32_t *src = &out_buf[y * half_w];
        uint32_t *dst = &blur_tmp[y * half_w];

        uint32_t r_sum = 0, g_sum = 0, b_sum = 0;
        int count = 0;

        for (int dx = -radius; dx <= radius && dx < half_w; dx++) {
            if (dx >= 0) {
                uint32_t c = src[dx];
                r_sum += (c>>16)&0xFF;
                g_sum += (c>>8)&0xFF;
                b_sum += c&0xFF;
                count++;
            }
        }

        for (int x = 0; x < half_w; x++) {
            dst[x] = 0xFF000000 | ((r_sum/count) << 16) | ((g_sum/count) << 8) | (b_sum/count);

            int remove_x = x - radius;
            int add_x = x + radius + 1;

            if (remove_x >= 0) {
                uint32_t c = src[remove_x];
                r_sum -= (c>>16)&0xFF;
                g_sum -= (c>>8)&0xFF;
                b_sum -= c&0xFF;
                count--;
            }
            if (add_x < half_w) {
                uint32_t c = src[add_x];
                r_sum += (c>>16)&0xFF;
                g_sum += (c>>8)&0xFF;
                b_sum += c&0xFF;
                count++;
            }
        }
    }

    // Step 4: Vertical blur pass (GPU fragment shader simulation)
    for (int x = 0; x < half_w; x++) {
        uint32_t r_sum = 0, g_sum = 0, b_sum = 0;
        int count = 0;

        for (int dy = -radius; dy <= radius && dy < half_h; dy++) {
            if (dy >= 0) {
                uint32_t c = blur_tmp[dy * half_w + x];
                r_sum += (c>>16)&0xFF;
                g_sum += (c>>8)&0xFF;
                b_sum += c&0xFF;
                count++;
            }
        }

        for (int y = 0; y < half_h; y++) {
            out_buf[y * half_w + x] = 0xFF000000 | ((r_sum/count) << 16) | ((g_sum/count) << 8) | (b_sum/count);

            int remove_y = y - radius;
            int add_y = y + radius + 1;

            if (remove_y >= 0) {
                uint32_t c = blur_tmp[remove_y * half_w + x];
                r_sum -= (c>>16)&0xFF;
                g_sum -= (c>>8)&0xFF;
                b_sum -= c&0xFF;
                count--;
            }
            if (add_y < half_h) {
                uint32_t c = blur_tmp[add_y * half_w + x];
                r_sum += (c>>16)&0xFF;
                g_sum += (c>>8)&0xFF;
                b_sum += c&0xFF;
                count++;
            }
        }
    }

    free(blur_tmp);
    return 0;
}
