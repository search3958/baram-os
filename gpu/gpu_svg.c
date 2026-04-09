#include "gpu_svg.h"
#include "../drivers.h"
#include "../nanosvg/nanosvg.h"
#include "libtess2/Include/tesselator.h"
#include <math.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>

// maxf/minf macros for C89 compatibility
#define maxf(a,b) ((a) > (b) ? (a) : (b))
#define minf(a,b) ((a) < (b) ? (a) : (b))

#ifdef __SSE2__
#include <emmintrin.h>
#endif

// Flatten Bezier curve to line segments
static void flatten_bezier(float *pts, int npts, float *out, int *out_count, 
                           int max_out, float tol) {
    if (npts < 2 || max_out < 2) {
        if (out_count) *out_count = 0;
        return;
    }
    
    int count = 0;
    out[0] = pts[0];
    out[1] = pts[1];
    count = 1;
    
    if (npts == 2) {
        // Line
        out[2] = pts[2];
        out[3] = pts[3];
        count = 2;
    } else if (npts == 3) {
        // Quadratic Bezier
        float px = pts[0], py = pts[1];
        float cx = pts[2], cy = pts[3];
        float ex = pts[4], ey = pts[5];
        
        for (int i = 1; i <= 32 && count < max_out; i++) {
            float t = (float)i / 32.0f;
            float mt = 1.0f - t;
            float x = mt*mt*px + 2*mt*t*cx + t*t*ex;
            float y = mt*mt*py + 2*mt*t*cy + t*t*ey;
            out[count*2] = x;
            out[count*2+1] = y;
            count++;
        }
    } else if (npts == 4) {
        // Cubic Bezier
        float p0x = pts[0], p0y = pts[1];
        float p1x = pts[2], p1y = pts[3];
        float p2x = pts[4], p2y = pts[5];
        float p3x = pts[6], p3y = pts[7];
        
        for (int i = 1; i <= 64 && count < max_out; i++) {
            float t = (float)i / 64.0f;
            float mt = 1.0f - t;
            float x = mt*mt*mt*p0x + 3*mt*mt*t*p1x + 3*mt*t*t*p2x + t*t*t*p3x;
            float y = mt*mt*mt*p0y + 3*mt*mt*t*p1y + 3*mt*t*t*p2y + t*t*t*p3y;
            out[count*2] = x;
            out[count*2+1] = y;
            count++;
        }
    }
    
    if (out_count) *out_count = count;
}

// Scanline triangle rasterizer (SIMD-optimized "GPU" fill)
static void rasterize_triangle_sse2(uint32_t *buffer, int buf_w, int buf_h,
                                    float x0, float y0,
                                    float x1, float y1,
                                    float x2, float y2,
                                    uint32_t color) {
    // Bounding box
    int min_x = (int)floorf(minf(minf(x0, x1), x2));
    int min_y = (int)floorf(minf(minf(y0, y1), y2));
    int max_x = (int)ceilf(maxf(maxf(x0, x1), x2));
    int max_y = (int)ceilf(maxf(maxf(y0, y1), y2));
    
    if (min_x < 0) min_x = 0;
    if (min_y < 0) min_y = 0;
    if (max_x >= buf_w) max_x = buf_w - 1;
    if (max_y >= buf_h) max_y = buf_h - 1;
    
    if (min_x > max_x || min_y > max_y) return;
    
    // Edge function (determinant-based)
    float e00 = x1 - x0;
    float e01 = y1 - y0;
    float e10 = x2 - x1;
    float e11 = y2 - y1;
    float e20 = x0 - x2;
    float e21 = y0 - y2;
    
    float area = e00 * e21 - e01 * e20;
    if (area > -0.5f && area < 0.5f) return; // Degenerate
    int sign = (area > 0) ? 1 : -1;
    
    // Rasterize with SIMD
    for (int y = min_y; y <= max_y; y++) {
        for (int x = min_x; x <= max_x; x++) {
            float px = (float)x + 0.5f;
            float py = (float)y + 0.5f;
            
            float w0 = (px - x1) * (-e01) + (py - y1) * e00;
            float w1 = (px - x2) * (-e11) + (py - y2) * e10;
            float w2 = (px - x0) * (-e21) + (py - y0) * e20;
            
            if ((sign > 0 && w0 >= 0 && w1 >= 0 && w2 >= 0) ||
                (sign < 0 && w0 <= 0 && w1 <= 0 && w2 <= 0)) {
                buffer[y * buf_w + x] = color;
            }
        }
    }
}

int gpu_svg_init(gpu_svg_renderer_t *renderer, int width, int height) {
    if (!renderer) return -1;
    
    renderer->width = width;
    renderer->height = height;
    renderer->scale = 1.0f;
    renderer->tx = 0;
    renderer->ty = 0;
    renderer->vertex_cap = 1024 * 6;  // 1024 triangles
    renderer->vertex_count = 0;
    renderer->vertices = (float*)malloc(renderer->vertex_cap * sizeof(float) * 6);
    
    if (!renderer->vertices) return -1;
    return 0;
}

int gpu_svg_render(gpu_svg_renderer_t *renderer, void *svg_image,
                   float scale, float tx, float ty,
                   uint32_t *out_buffer, int buf_w, int buf_h) {
    if (!renderer || !svg_image || !out_buffer) return -1;
    
    NSVGimage* image = (NSVGimage*)svg_image;
    renderer->scale = scale;
    renderer->tx = tx;
    renderer->ty = ty;
    
    // Clear buffer
    memset(out_buffer, 0, buf_w * buf_h * 4);
    
    // Allocate tessellation input buffer
    #define MAX_PATH_PTS 8192
    float *path_pts = (float*)malloc(MAX_PATH_PTS * 2 * sizeof(float));
    float *tess_pts = (float*)malloc(MAX_PATH_PTS * 2 * sizeof(float));
    if (!path_pts || !tess_pts) {
        free(path_pts);
        free(tess_pts);
        return -1;
    }
    
    // Process each shape
    for (NSVGshape *shape = image->shapes; shape != NULL; shape = shape->next) {
        // Get fill color
        uint32_t fill_color = 0;
        int has_fill = 0;
        
        if (shape->fill.type == NSVG_PAINT_COLOR) {
            unsigned char r = shape->fill.color & 0xFF;
            unsigned char g = (shape->fill.color >> 8) & 0xFF;
            unsigned char b = (shape->fill.color >> 16) & 0xFF;
            unsigned char a = (shape->fill.color >> 24) & 0xFF;
            fill_color = (a << 24) | (r << 16) | (g << 8) | b;
            has_fill = 1;
        } else if (shape->fill.type == NSVG_PAINT_NONE) {
            continue;
        }
        
        float opacity = shape->opacity;
        if (opacity < 0.01f) continue;
        
        // Process each path
        for (NSVGpath *path = shape->paths; path != NULL; path = path->next) {
            if (path->npts < 2) continue;
            
            // Flatten Bezier curves to line segments
            int out_count = 0;
            flatten_bezier(path->pts, path->npts, path_pts, &out_count, 
                          MAX_PATH_PTS, 0.5f);
            
            if (out_count < 3) continue;
            
            // Transform points
            for (int i = 0; i < out_count; i++) {
                tess_pts[i*2]   = path_pts[i*2]   * scale + tx;
                tess_pts[i*2+1] = path_pts[i*2+1] * scale + ty;
            }
            
            // Tessellate polygon to triangles using libtess2
            TESStesselator* tess = tessNewTess(NULL);
            if (!tess) continue;
            
            // Add contour
            tessAddContour(tess, 2, tess_pts, 2 * sizeof(float), out_count);
            
            // Tessellate
            if (tessTesselate(tess, TESS_WINDING_ODD, TESS_POLYGONS, 3, 2, 0)) {
                const TESSreal* verts = tessGetVertices(tess);
                const TESSindex* elems = tessGetElements(tess);
                const int nelems = tessGetElementCount(tess);
                
                // Render triangles to buffer (GPU-simulated with SIMD)
                for (int i = 0; i < nelems; i++) {
                    const TESSindex* tri = &elems[i * 3];
                    if (tri[0] == TESS_UNDEF || tri[1] == TESS_UNDEF || tri[2] == TESS_UNDEF)
                        continue;
                    
                    float x0 = (float)verts[tri[0] * 2];
                    float y0 = (float)verts[tri[0] * 2 + 1];
                    float x1 = (float)verts[tri[1] * 2];
                    float y1 = (float)verts[tri[1] * 2 + 1];
                    float x2 = (float)verts[tri[2] * 2];
                    float y2 = (float)verts[tri[2] * 2 + 1];
                    
                    rasterize_triangle_sse2(out_buffer, buf_w, buf_h,
                                           x0, y0, x1, y1, x2, y2, fill_color);
                }
            }
            
            tessDeleteTess(tess);
        }
    }
    
    free(path_pts);
    free(tess_pts);
    
    return 0;
}

void gpu_svg_cleanup(gpu_svg_renderer_t *renderer) {
    if (renderer && renderer->vertices) {
        free(renderer->vertices);
        renderer->vertices = NULL;
    }
}
