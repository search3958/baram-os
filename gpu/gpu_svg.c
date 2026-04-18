#include "gpu_svg.h"
#include "../drivers.h"
#include "../nanosvg/nanosvg.h"
#include "../nanosvg/nanosvgrast.h"
#include <math.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

// Note: NANOSVGRAST_IMPLEMENTATION is defined in kernel.c, 
// so we don't define it here to avoid duplicate symbols.

int gpu_svg_init(gpu_svg_renderer_t *renderer, int width, int height) {
    if (!renderer) return -1;
    
    renderer->width = width;
    renderer->height = height;
    renderer->scale = 1.0f;
    renderer->tx = 0;
    renderer->ty = 0;
    renderer->vertex_cap = 0;
    renderer->vertex_count = 0;
    renderer->vertices = NULL;
    
    return 0;
}

int gpu_svg_render(gpu_svg_renderer_t *renderer, void *svg_image,
                   float scale, float tx, float ty,
                   uint32_t *out_buffer, int buf_w, int buf_h) {
    if (!renderer || !svg_image || !out_buffer) return -1;
    
    NSVGimage* image = (NSVGimage*)svg_image;
    
    // Clear buffer
    memset(out_buffer, 0, buf_w * buf_h * 4);
    
    // Create rasterizer
    NSVGrasterizer* rast = nsvgCreateRasterizer();
    if (!rast) return -1;
    
    // Rasterize SVG image directly to the buffer
    // nanosvgrast writes RGBA 32-bit.
    nsvgRasterize(rast, image, tx, ty, scale, (unsigned char*)out_buffer, buf_w, buf_h, buf_w * 4);
    
    nsvgDeleteRasterizer(rast);
    
    return 0;
}

void gpu_svg_cleanup(gpu_svg_renderer_t *renderer) {
    if (renderer && renderer->vertices) {
        free(renderer->vertices);
        renderer->vertices = NULL;
    }
}
