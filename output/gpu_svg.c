#include <stdint.h>
typedef struct { float *vertices; int vertex_count; int vertex_cap; uint32_t width; uint32_t height; float scale; float tx, ty; } gpu_svg_renderer_t;
int gpu_svg_init(gpu_svg_renderer_t *r, int w, int h) { return 0; }
int gpu_svg_render(gpu_svg_renderer_t *r, void *img, float s, float tx, float ty, uint32_t *buf, int bw, int bh) { return 0; }
void gpu_svg_cleanup(gpu_svg_renderer_t *r) {}
