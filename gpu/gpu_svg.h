#ifndef GPU_SVG_H
#define GPU_SVG_H

#include <stdint.h>
#include <stddef.h>

typedef struct gpu_svg_document gpu_svg_document_t;

typedef struct {
    float *vertices;    // Triangle vertices (x, y, r, g, b, a)
    int vertex_count;
    int vertex_cap;
    
    uint32_t width;
    uint32_t height;
    float scale;
    float tx, ty;
} gpu_svg_renderer_t;

#ifdef __cplusplus
extern "C" {
#endif

// Initialize GPU SVG renderer
int gpu_svg_init(gpu_svg_renderer_t *renderer, int width, int height);

gpu_svg_document_t *gpu_svg_parse(const char *svg_data);
void gpu_svg_delete(gpu_svg_document_t *document);
float gpu_svg_width(const gpu_svg_document_t *document);
float gpu_svg_height(const gpu_svg_document_t *document);

int gpu_svg_rasterize(const gpu_svg_document_t *document,
                      float scale, float tx, float ty,
                      unsigned char *out_rgba,
                      int buf_w, int buf_h, int stride);

int gpu_svg_render(gpu_svg_renderer_t *renderer, const gpu_svg_document_t *document,
                   float scale, float tx, float ty,
                   uint32_t *out_buffer, int buf_w, int buf_h);

// Cleanup
void gpu_svg_cleanup(gpu_svg_renderer_t *renderer);

#ifdef __cplusplus
}
#endif

#endif // GPU_SVG_H
