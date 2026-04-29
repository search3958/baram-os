#include "warp_draw.h"

#include <math.h>
#include <stddef.h>
#include <string.h>

#define WARP_DRAW_MAX_POINTS 96

typedef struct {
  float x;
  float y;
} warp_draw_point_t;

static const float SQUIRCLE_KX[] = {1.498f, 3.381f, 7.456f, 12.630f, 17.368f, 21.770f, 30.573f};
static const float SQUIRCLE_KY[] = {0.800f, 3.600f, 7.370f, 12.544f, 16.619f, 18.502f, 20.000f};

static void blend_pixel_argb_premul(uint32_t *px,
                                    unsigned char sr, unsigned char sg, unsigned char sb, unsigned char sa) {
  if (sa == 0) return;

  uint32_t dst = *px;
  uint32_t dst_a = (dst >> 24) & 0xFFu;
  uint32_t dst_r = (dst >> 16) & 0xFFu;
  uint32_t dst_g = (dst >> 8) & 0xFFu;
  uint32_t dst_b = dst & 0xFFu;
  uint32_t inv_sa = 255u - sa;

  uint32_t src_r = ((uint32_t)sr * sa + 127u) / 255u;
  uint32_t src_g = ((uint32_t)sg * sa + 127u) / 255u;
  uint32_t src_b = ((uint32_t)sb * sa + 127u) / 255u;
  uint32_t out_a = sa + ((dst_a * inv_sa + 127u) / 255u);
  uint32_t out_r = src_r + ((dst_r * inv_sa + 127u) / 255u);
  uint32_t out_g = src_g + ((dst_g * inv_sa + 127u) / 255u);
  uint32_t out_b = src_b + ((dst_b * inv_sa + 127u) / 255u);

  if (out_a > 255u) out_a = 255u;
  if (out_r > 255u) out_r = 255u;
  if (out_g > 255u) out_g = 255u;
  if (out_b > 255u) out_b = 255u;
  *px = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
}

static void resolve_squircle_geometry(int w, int h, float radius,
                                      float *edge_x, float *edge_y, float *curve_scale) {
  float fw = (float)w;
  float fh = (float)h;
  float min_edge = (fw < fh) ? fw : fh;
  float max_possible_radius = min_edge / 2.0f;

  if (radius == -1.0f) {
    float s = (fh >= 46.0f) ? 1.15f : 1.0f;
    *curve_scale = s;
    *edge_x = SQUIRCLE_KX[6] * s;
    *edge_y = SQUIRCLE_KY[6] * s;
    if (*edge_x > fw / 2.0f) *edge_x = fw / 2.0f;
    if (*edge_y > fh / 2.0f) *edge_y = fh / 2.0f;
    return;
  }

  float radius_px;
  if (radius >= 1000.0f) {
    float radius_pct = radius - 1000.0f;
    if (radius_pct > 100.0f) radius_pct = 100.0f;
    if (radius_pct < 0.0f) radius_pct = 0.0f;
    radius_px = (max_possible_radius * radius_pct) / 100.0f;
  } else {
    radius_px = radius;
  }
  if (radius_px > max_possible_radius) radius_px = max_possible_radius;

  float s = (fh >= 46.0f) ? 1.15f : 1.0f;
  float default_radius = 12.0f * s;
  float scale = (default_radius > 0.0f) ? (radius_px / default_radius) : 1.0f;
  float max_scale_x = (fw / 2.0f) / (SQUIRCLE_KX[6] * s);
  float max_scale_y = (fh / 2.0f) / (SQUIRCLE_KY[6] * s);
  float max_scale = (max_scale_x < max_scale_y) ? max_scale_x : max_scale_y;
  if (scale > max_scale) scale = max_scale;
  if (scale < 0.0f) scale = 0.0f;

  *curve_scale = s * scale;
  *edge_x = SQUIRCLE_KX[6] * *curve_scale;
  *edge_y = SQUIRCLE_KY[6] * *curve_scale;
  if (*edge_x > fw / 2.0f) *edge_x = fw / 2.0f;
  if (*edge_y > fh / 2.0f) *edge_y = fh / 2.0f;
}

static warp_draw_point_t cubic_point(float t, warp_draw_point_t p0, warp_draw_point_t p1,
                                     warp_draw_point_t p2, warp_draw_point_t p3) {
  float mt = 1.0f - t;
  float mt2 = mt * mt;
  float t2 = t * t;
  warp_draw_point_t p;
  p.x = mt2 * mt * p0.x + 3.0f * mt2 * t * p1.x + 3.0f * mt * t2 * p2.x + t2 * t * p3.x;
  p.y = mt2 * mt * p0.y + 3.0f * mt2 * t * p1.y + 3.0f * mt * t2 * p2.y + t2 * t * p3.y;
  return p;
}

static void push_point(warp_draw_point_t *pts, int *count, warp_draw_point_t p) {
  if (*count < WARP_DRAW_MAX_POINTS) pts[(*count)++] = p;
}

static void append_cubic(warp_draw_point_t *pts, int *count,
                         warp_draw_point_t p0, warp_draw_point_t p1,
                         warp_draw_point_t p2, warp_draw_point_t p3) {
  for (int i = 1; i <= 8; ++i) {
    float t = (float)i / 8.0f;
    push_point(pts, count, cubic_point(t, p0, p1, p2, p3));
  }
}

static int build_squircle_polygon(float x, float y, int w, int h, float radius,
                                  warp_draw_point_t *pts) {
  int count = 0;
  float fw = (float)w;
  float fh = (float)h;
  float edge_x, edge_y, s;
  resolve_squircle_geometry(w, h, radius, &edge_x, &edge_y, &s);

  push_point(pts, &count, (warp_draw_point_t){x + fw, y + fh / 2.0f});
  push_point(pts, &count, (warp_draw_point_t){x + fw, y + fh - edge_y});
  append_cubic(pts, &count, (warp_draw_point_t){x + fw, y + fh - edge_y},
               (warp_draw_point_t){x + fw, y + fh - edge_y + SQUIRCLE_KY[0] * s},
               (warp_draw_point_t){x + fw, y + fh - edge_y + SQUIRCLE_KY[1] * s},
               (warp_draw_point_t){x + fw - SQUIRCLE_KX[0] * s, y + fh - edge_y + SQUIRCLE_KY[2] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + fw - SQUIRCLE_KX[0] * s, y + fh - edge_y + SQUIRCLE_KY[2] * s},
               (warp_draw_point_t){x + fw - SQUIRCLE_KX[1] * s, y + fh - edge_y + SQUIRCLE_KY[3] * s},
               (warp_draw_point_t){x + fw - SQUIRCLE_KX[2] * s, y + fh - edge_y + SQUIRCLE_KY[4] * s},
               (warp_draw_point_t){x + fw - SQUIRCLE_KX[3] * s, y + fh - edge_y + SQUIRCLE_KY[5] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + fw - SQUIRCLE_KX[3] * s, y + fh - edge_y + SQUIRCLE_KY[5] * s},
               (warp_draw_point_t){x + fw - SQUIRCLE_KX[4] * s, y + fh},
               (warp_draw_point_t){x + fw - SQUIRCLE_KX[5] * s, y + fh},
               (warp_draw_point_t){x + fw - edge_x, y + fh});
  push_point(pts, &count, (warp_draw_point_t){x + edge_x, y + fh});
  append_cubic(pts, &count, (warp_draw_point_t){x + edge_x, y + fh},
               (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[5]) * s, y + fh},
               (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[4]) * s, y + fh},
               (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + fh - edge_y + SQUIRCLE_KY[5] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + fh - edge_y + SQUIRCLE_KY[5] * s},
               (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[2]) * s, y + fh - edge_y + SQUIRCLE_KY[4] * s},
               (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[1]) * s, y + fh - edge_y + SQUIRCLE_KY[3] * s},
               (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + fh - edge_y + SQUIRCLE_KY[2] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + fh - edge_y + SQUIRCLE_KY[2] * s},
               (warp_draw_point_t){x, y + fh - edge_y + SQUIRCLE_KY[1] * s},
               (warp_draw_point_t){x, y + fh - edge_y + SQUIRCLE_KY[0] * s},
               (warp_draw_point_t){x, y + fh - edge_y});
  push_point(pts, &count, (warp_draw_point_t){x, y + edge_y});
  append_cubic(pts, &count, (warp_draw_point_t){x, y + edge_y},
               (warp_draw_point_t){x, y + edge_y - SQUIRCLE_KY[0] * s},
               (warp_draw_point_t){x, y + edge_y - SQUIRCLE_KY[1] * s},
               (warp_draw_point_t){x + SQUIRCLE_KX[0] * s, y + edge_y - SQUIRCLE_KY[2] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + SQUIRCLE_KX[0] * s, y + edge_y - SQUIRCLE_KY[2] * s},
               (warp_draw_point_t){x + SQUIRCLE_KX[1] * s, y + edge_y - SQUIRCLE_KY[3] * s},
               (warp_draw_point_t){x + SQUIRCLE_KX[2] * s, y + edge_y - SQUIRCLE_KY[4] * s},
               (warp_draw_point_t){x + SQUIRCLE_KX[3] * s, y + edge_y - SQUIRCLE_KY[5] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + SQUIRCLE_KX[3] * s, y + edge_y - SQUIRCLE_KY[5] * s},
               (warp_draw_point_t){x + SQUIRCLE_KX[4] * s, y},
               (warp_draw_point_t){x + SQUIRCLE_KX[5] * s, y},
               (warp_draw_point_t){x + edge_x, y});
  push_point(pts, &count, (warp_draw_point_t){x + fw - edge_x, y});
  append_cubic(pts, &count, (warp_draw_point_t){x + fw - edge_x, y},
               (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[5]) * s, y},
               (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[4]) * s, y},
               (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + edge_y - SQUIRCLE_KY[5] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + edge_y - SQUIRCLE_KY[5] * s},
               (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[2]) * s, y + edge_y - SQUIRCLE_KY[4] * s},
               (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[1]) * s, y + edge_y - SQUIRCLE_KY[3] * s},
               (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + edge_y - SQUIRCLE_KY[2] * s});
  append_cubic(pts, &count, (warp_draw_point_t){x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + edge_y - SQUIRCLE_KY[2] * s},
               (warp_draw_point_t){x + fw, y + edge_y - SQUIRCLE_KY[1] * s},
               (warp_draw_point_t){x + fw, y + edge_y - SQUIRCLE_KY[0] * s},
               (warp_draw_point_t){x + fw, y + edge_y});
  return count;
}

static int point_in_polygon(float x, float y, const warp_draw_point_t *pts, int n) {
  int inside = 0;
  if (n < 3) return 0;
  for (int i = 0, j = n - 1; i < n; j = i++) {
    const warp_draw_point_t a = pts[i];
    const warp_draw_point_t b = pts[j];
    int intersect = ((a.y > y) != (b.y > y)) &&
                    (x < (b.x - a.x) * (y - a.y) / (((b.y - a.y) == 0.0f) ? 1e-6f : (b.y - a.y)) + a.x);
    if (intersect) inside = !inside;
  }
  return inside;
}

static unsigned char polygon_coverage(int px, int py, const warp_draw_point_t *pts, int n) {
  static const float offsets[4][2] = {{0.25f,0.25f},{0.75f,0.25f},{0.25f,0.75f},{0.75f,0.75f}};
  int hits = 0;
  for (int i = 0; i < 4; ++i) {
    if (point_in_polygon((float)px + offsets[i][0], (float)py + offsets[i][1], pts, n)) hits++;
  }
  return (unsigned char)((hits * 255) / 4);
}

static void rasterize_line(const warp_draw_op_t *op, float scale, float tx, float ty,
                           unsigned char *out, int buf_w, int buf_h, int stride) {
  if (op->sa == 0 || op->stroke_width <= 0.0f) return;
  float x1 = op->x * scale + tx;
  float y1 = op->y * scale + ty;
  float x2 = op->x2 * scale + tx;
  float y2 = op->y2 * scale + ty;
  float half_w = op->stroke_width * scale * 0.5f;
  if (half_w < 0.5f) half_w = 0.5f;

  int min_x = (int)floorf(((x1 < x2) ? x1 : x2) - half_w - 1.0f);
  int max_x = (int)ceilf(((x1 > x2) ? x1 : x2) + half_w + 1.0f);
  int min_y = (int)floorf(((y1 < y2) ? y1 : y2) - half_w - 1.0f);
  int max_y = (int)ceilf(((y1 > y2) ? y1 : y2) + half_w + 1.0f);
  if (min_x < 0) min_x = 0;
  if (min_y < 0) min_y = 0;
  if (max_x > buf_w) max_x = buf_w;
  if (max_y > buf_h) max_y = buf_h;
  if (min_x >= max_x || min_y >= max_y) return;

  float dx = x2 - x1;
  float dy = y2 - y1;
  float len2 = dx * dx + dy * dy;
  if (len2 <= 0.0001f) return;

  for (int y = min_y; y < max_y; ++y) {
    uint32_t *row = (uint32_t *)(out + (size_t)y * (size_t)stride);
    for (int x = min_x; x < max_x; ++x) {
      float px = (float)x + 0.5f;
      float py = (float)y + 0.5f;
      float t = ((px - x1) * dx + (py - y1) * dy) / len2;
      if (t < 0.0f) t = 0.0f;
      if (t > 1.0f) t = 1.0f;
      float cx = x1 + t * dx;
      float cy = y1 + t * dy;
      float dist = sqrtf((px - cx) * (px - cx) + (py - cy) * (py - cy));
      float cov = half_w + 1.0f - dist;
      if (cov <= 0.0f) continue;
      if (cov > 1.0f) cov = 1.0f;
      unsigned char a = (unsigned char)((float)op->sa * cov + 0.5f);
      blend_pixel_argb_premul(&row[x], op->sr, op->sg, op->sb, a);
    }
  }
}

static void rasterize_squircle(const warp_draw_op_t *op, float scale, float tx, float ty,
                               unsigned char *out, int buf_w, int buf_h, int stride) {
  warp_draw_point_t outer[WARP_DRAW_MAX_POINTS];
  warp_draw_point_t inner[WARP_DRAW_MAX_POINTS];
  int sw = (int)(op->w * scale + 0.999f);
  int sh = (int)(op->h * scale + 0.999f);
  if (sw <= 0 || sh <= 0) return;
  int outer_count = build_squircle_polygon(op->x * scale + tx, op->y * scale + ty, sw, sh, op->radius, outer);
  int inner_count = 0;
  int ssw = (int)(op->stroke_width * scale + 0.5f);
  if (ssw < 1) ssw = 1;
  if (op->has_stroke && op->stroke_width > 0.0f && sw - ssw * 2 > 0 && sh - ssw * 2 > 0) {
    inner_count = build_squircle_polygon(op->x * scale + tx + (float)ssw,
                                         op->y * scale + ty + (float)ssw,
                                         sw - ssw * 2, sh - ssw * 2, op->radius, inner);
  }

  int min_x = (int)(op->x * scale + tx);
  int min_y = (int)(op->y * scale + ty);
  int max_x = min_x + sw;
  int max_y = min_y + sh;
  if (min_x < 0) min_x = 0;
  if (min_y < 0) min_y = 0;
  if (max_x > buf_w) max_x = buf_w;
  if (max_y > buf_h) max_y = buf_h;

  for (int y = min_y; y < max_y; ++y) {
    uint32_t *row = (uint32_t *)(out + (size_t)y * (size_t)stride);
    for (int x = min_x; x < max_x; ++x) {
      unsigned char outer_cov = polygon_coverage(x, y, outer, outer_count);
      if (outer_cov == 0) continue;
      unsigned char inner_cov = inner_count ? polygon_coverage(x, y, inner, inner_count) : 0;
      if (op->has_fill) {
        unsigned char a = (unsigned char)(((unsigned)op->fa * outer_cov + 127u) / 255u);
        blend_pixel_argb_premul(&row[x], op->fr, op->fg, op->fb, a);
      }
      if (op->has_stroke && outer_cov > inner_cov) {
        unsigned char edge = (unsigned char)(outer_cov - inner_cov);
        unsigned char a = (unsigned char)(((unsigned)op->sa * edge + 127u) / 255u);
        blend_pixel_argb_premul(&row[x], op->sr, op->sg, op->sb, a);
      }
    }
  }
}

int warp_draw_rasterize_premul(const warp_draw_op_t *ops, int op_count,
                               float scale, float tx, float ty,
                               unsigned char *out_argb_premul,
                               int buf_w, int buf_h, int stride) {
  if (!ops || op_count < 0 || !out_argb_premul) return -1;
  if (buf_w <= 0 || buf_h <= 0 || stride < buf_w * 4) return -1;

  memset(out_argb_premul, 0, (size_t)stride * (size_t)buf_h);
  for (int i = 0; i < op_count; ++i) {
    if (ops[i].type == WARP_DRAW_LINE) {
      rasterize_line(&ops[i], scale, tx, ty, out_argb_premul, buf_w, buf_h, stride);
    } else if (ops[i].type == WARP_DRAW_SQUIRCLE) {
      rasterize_squircle(&ops[i], scale, tx, ty, out_argb_premul, buf_w, buf_h, stride);
    }
  }
  return 0;
}
