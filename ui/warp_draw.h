#ifndef WARP_DRAW_H
#define WARP_DRAW_H

#include <stdint.h>

typedef enum {
  WARP_DRAW_SQUIRCLE = 1,
  WARP_DRAW_LINE = 2
} warp_draw_type_t;

typedef struct {
  warp_draw_type_t type;
  float x, y, w, h;
  float x2, y2;
  float radius;
  float stroke_width;
  uint8_t fr, fg, fb, fa;
  uint8_t sr, sg, sb, sa;
  uint8_t has_fill;
  uint8_t has_stroke;
} warp_draw_op_t;

int warp_draw_rasterize_premul(const warp_draw_op_t *ops,
                               int op_count,
                               float scale,
                               float tx,
                               float ty,
                               unsigned char *out_argb_premul,
                               int buf_w,
                               int buf_h,
                               int stride);

#endif
