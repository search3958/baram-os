#ifndef GPU_SVG_H
#define GPU_SVG_H

#include <stdint.h>

// GPU SVG path rasterizer
// 1. Uses nanosvg to parse SVG
// 2. Uses nanosvgrast to rasterize to buffer
// 3. (Future) Upload to GPU for acceleration

typedef struct {
    float *vertices;    // Triangle vertices (x, y, r, g, b, a)
    int vertex_count;
    int vertex_cap;
    
    uint32_t width;
    uint32_t height;
    float scale;
    float tx, ty;
} gpu_svg_renderer_t;

// Initialize GPU SVG renderer
int gpu_svg_init(gpu_svg_renderer_t *renderer, int width, int height);

// Render SVG from parsed nanosvg image to GPU
// This function:
// 1. Extracts Bezier paths from NSVGimage
// 2. Flattens to line segments
// 3. Tessellates to triangles
// 4. Uploads to GPU
int gpu_svg_render(gpu_svg_renderer_t *renderer, void *svg_image, 
                   float scale, float tx, float ty,
                   uint32_t *out_buffer, int buf_w, int buf_h);

// Cleanup
void gpu_svg_cleanup(gpu_svg_renderer_t *renderer);

#endif // GPU_SVG_H
