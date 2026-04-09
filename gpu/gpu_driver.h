#ifndef GPU_DRIVER_H
#define GPU_DRIVER_H

#include <stdint.h>
#include <stddef.h>

// GPU availability flag
extern int g_gpu_available;

// GPU resource IDs
typedef struct {
    uint32_t resource_id;
    uint32_t width;
    uint32_t height;
    void *cpu_ptr;      // CPU-side buffer
    size_t size;
} gpu_resource_t;

// GPU program (shader) handle
typedef struct {
    uint32_t program_id;
    int uniform_locations[16];
    char uniform_names[16][32];
    int uniform_count;
} gpu_program_t;

// GPU framebuffer
typedef struct {
    uint32_t fb_id;
    gpu_resource_t color_attachment;
    gpu_resource_t depth_attachment;
} gpu_framebuffer_t;

// GPU initialization
int gpu_init(void);

// Resource management
int gpu_create_resource(uint32_t width, uint32_t height, gpu_resource_t *out_resource);
void gpu_destroy_resource(gpu_resource_t *resource);
int gpu_upload_texture(gpu_resource_t *resource, const void *data, size_t size);
int gpu_download_texture(gpu_resource_t *resource, void *out_data, size_t size);

// Program management
int gpu_create_program(const char *vertex_shader, const char *fragment_shader, gpu_program_t *out_program);
int gpu_use_program(gpu_program_t *program);
int gpu_set_uniform_int(gpu_program_t *program, const char *name, int value);
int gpu_set_uniform_float(gpu_program_t *program, const char *name, float value);
int gpu_set_uniform_float2(gpu_program_t *program, const char *name, float x, float y);

// Framebuffer operations
int gpu_create_framebuffer(uint32_t width, uint32_t height, gpu_framebuffer_t *out_fb);
void gpu_destroy_framebuffer(gpu_framebuffer_t *fb);
int gpu_bind_framebuffer(gpu_framebuffer_t *fb);
int gpu_unbind_framebuffer(void);

// Rendering
void gpu_clear_color(float r, float g, float b, float a);
void gpu_draw_quad(float x, float y, float w, float h);
void gpu_flush(void);
void gpu_finish(void);  // Wait for GPU to complete

// Debug
const char* gpu_get_error(void);

#endif // GPU_DRIVER_H
