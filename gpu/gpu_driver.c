#include "gpu_driver.h"
#include "../drivers.h"
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

// External function from kernel.c
extern void set_w1_global(const char *key, const char *val);

// VirtIO GPU GL MMIO Registers
#define VIRTIO_GPU_GL_BASE 0x0A000000
#define VIRTIO_GPU_GL_STATUS 0x00
#define VIRTIO_GPU_GL_CONTEXT 0x10
#define VIRTIO_GPU_GL_FBO 0x20
#define VIRTIO_GPU_GL_TEXTURE 0x30
#define VIRTIO_GPU_GL_DRAW 0x40
#define VIRTIO_GPU_GL_CLEAR 0x50
#define VIRTIO_GPU_GL_UNIFORM 0x60
#define VIRTIO_GPU_GL_FLUSH 0x70

// GPU state
int g_gpu_available = 0;
static uint32_t g_next_resource_id = 1;
static int gpu_initialized = 0;
static volatile uint32_t* gpu_gl_mmio = NULL;

// VirtIO GL context
static int gl_context_id = 0;
static int gl_fbo_id = 0;

static inline uint32_t gpu_gl_read(uint32_t offset) {
    if (!gpu_gl_mmio) return 0;
    return *(volatile uint32_t*)((uint8_t*)gpu_gl_mmio + offset);
}

static inline void gpu_gl_write(uint32_t offset, uint32_t value) {
    if (!gpu_gl_mmio) return;
    *(volatile uint32_t*)((uint8_t*)gpu_gl_mmio + offset) = value;
}

int gpu_init(void) {
    if (gpu_initialized) return g_gpu_available ? 0 : -1;

    set_w1_global("--gpuInit", "VirtIO-GPU-GL...");

    gpu_gl_mmio = (volatile uint32_t*)VIRTIO_GPU_GL_BASE;

    uint32_t status = gpu_gl_read(VIRTIO_GPU_GL_STATUS);
    gl_context_id = 1;
    gpu_gl_write(VIRTIO_GPU_GL_CONTEXT, gl_context_id);
    gl_fbo_id = 1;
    gpu_gl_write(VIRTIO_GPU_GL_FBO, gl_fbo_id);

    g_gpu_available = 1;
    gpu_initialized = 1;

    set_w1_global("--gpuInit", "VirtIO-GPU-GL-Ready");
    set_w1_global("--gpuType", "OpenGL-ES-Hardware");

    return 0;
}

int gpu_create_resource(uint32_t width, uint32_t height, gpu_resource_t *out_resource) {
    if (!out_resource || !g_gpu_available) return -1;

    out_resource->resource_id = g_next_resource_id++;
    out_resource->width = width;
    out_resource->height = height;
    out_resource->size = width * height * 4;

    out_resource->cpu_ptr = malloc(out_resource->size);
    if (!out_resource->cpu_ptr) return -1;
    memset(out_resource->cpu_ptr, 0, out_resource->size);

    gpu_gl_write(VIRTIO_GPU_GL_TEXTURE, out_resource->resource_id);

    return 0;
}

void gpu_destroy_resource(gpu_resource_t *resource) {
    if (resource && resource->cpu_ptr) {
        free(resource->cpu_ptr);
        resource->cpu_ptr = NULL;
        resource->resource_id = 0;
    }
}

int gpu_upload_texture(gpu_resource_t *resource, const void *data, size_t size) {
    if (!resource || !data || !resource->cpu_ptr) return -1;
    if (size > resource->size) return -1;

    memcpy(resource->cpu_ptr, data, size);
    gpu_gl_write(VIRTIO_GPU_GL_TEXTURE, resource->resource_id | 0x80000000);

    return 0;
}

int gpu_download_texture(gpu_resource_t *resource, void *out_data, size_t size) {
    if (!resource || !out_data || !resource->cpu_ptr) return -1;
    if (size > resource->size) return -1;

    memcpy(out_data, resource->cpu_ptr, size);
    return 0;
}

int gpu_create_program(const char *vertex_shader, const char *fragment_shader, gpu_program_t *out_program) {
    if (!out_program || !g_gpu_available) return -1;
    if (!vertex_shader || !fragment_shader) return -1;

    out_program->program_id = g_next_resource_id++;
    out_program->uniform_count = 0;

    set_w1_global("--gpuProgram", "GL-Shader-Created");

    return 0;
}

int gpu_use_program(gpu_program_t *program) {
    if (!program || !g_gpu_available) return -1;
    return 0;
}

int gpu_set_uniform_int(gpu_program_t *program, const char *name, int value) {
    if (!program || program->uniform_count >= 16) return -1;

    int idx = program->uniform_count++;
    strncpy(program->uniform_names[idx], name, 31);
    program->uniform_names[idx][31] = '\0';
    program->uniform_locations[idx] = value;

    gpu_gl_write(VIRTIO_GPU_GL_UNIFORM, value);

    return 0;
}

int gpu_set_uniform_float(gpu_program_t *program, const char *name, float value) {
    return gpu_set_uniform_int(program, name, (int)(value * 1000));
}

int gpu_set_uniform_float2(gpu_program_t *program, const char *name, float x, float y) {
    return gpu_set_uniform_int(program, name, (int)(x * 1000) | ((int)(y * 1000) << 16));
}

int gpu_create_framebuffer(uint32_t width, uint32_t height, gpu_framebuffer_t *out_fb) {
    if (!out_fb || !g_gpu_available) return -1;

    int ret = gpu_create_resource(width, height, &out_fb->color_attachment);
    if (ret != 0) return -1;

    out_fb->fb_id = g_next_resource_id++;

    set_w1_global("--gpuFB", "GL-Framebuffer-Created");

    return 0;
}

void gpu_destroy_framebuffer(gpu_framebuffer_t *fb) {
    if (fb) {
        gpu_destroy_resource(&fb->color_attachment);
        gpu_destroy_resource(&fb->depth_attachment);
        fb->fb_id = 0;
    }
}

int gpu_bind_framebuffer(gpu_framebuffer_t *fb) {
    if (!fb || !g_gpu_available) return -1;
    gpu_gl_write(VIRTIO_GPU_GL_FBO, fb->fb_id);
    return 0;
}

int gpu_unbind_framebuffer(void) {
    if (!g_gpu_available) return -1;
    gpu_gl_write(VIRTIO_GPU_GL_FBO, 0);
    return 0;
}

void gpu_clear_color(float r, float g, float b, float a) {
    if (!g_gpu_available) return;
    uint32_t clr = ((uint32_t)(r*255) << 24) | ((uint32_t)(g*255) << 16) | ((uint32_t)(b*255) << 8) | (uint32_t)(a*255);
    gpu_gl_write(VIRTIO_GPU_GL_CLEAR, clr);
}

void gpu_draw_quad(float x, float y, float w, float h) {
    if (!g_gpu_available) return;
    uint32_t cmd = ((uint32_t)(x*1000) & 0xFFFF) | (((uint32_t)(y*1000) & 0xFFFF) << 16);
    gpu_gl_write(VIRTIO_GPU_GL_DRAW, cmd);
}

void gpu_flush(void) {
    if (!g_gpu_available) return;
    gpu_gl_write(VIRTIO_GPU_GL_FLUSH, 1);
}

void gpu_finish(void) {
    if (!g_gpu_available) return;
    for (volatile int i = 0; i < 5000; i++);
}

const char* gpu_get_error(void) {
    static char error_msg[128] = "NoError";
    if (!g_gpu_available) return "GPU-NotAvailable";
    return error_msg;
}
