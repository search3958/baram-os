#ifndef GPU_BLUR_H
#define GPU_BLUR_H

#include "gpu_driver.h"
#include <stdint.h>

// GPU blur context
typedef struct {
    gpu_program_t downsample_program;
    gpu_program_t hblur_program;
    gpu_program_t vblur_program;
    
    gpu_framebuffer_t blur_fb[2];  // Ping-pong framebuffers
    gpu_resource_t input_texture;
    gpu_resource_t output_texture;
    
    int width;
    int height;
    int radius;
} gpu_blur_context_t;

// Initialize GPU blur
int gpu_blur_init(gpu_blur_context_t *ctx, int width, int height, int radius);

// Cleanup GPU blur resources
void gpu_blur_cleanup(gpu_blur_context_t *ctx);

// Execute GPU-accelerated blur
// Input: desktop_composite_buf (full resolution)
// Output: out_buf (half resolution, blurred)
int gpu_blur_execute(gpu_blur_context_t *ctx, 
                     const uint32_t *input_data, 
                     uint32_t *out_buf,
                     int input_width, int input_height);

// GPU-accelerated downsample (2x2 average)
int gpu_downsample_execute(gpu_blur_context_t *ctx,
                           const uint32_t *input_data,
                           uint32_t *out_buf,
                           int input_width, int input_height);

#endif // GPU_BLUR_H
