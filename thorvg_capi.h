#ifndef __THORVG_CAPI_H__
#define __THORVG_CAPI_H__

#include <stdbool.h>
#include <stdint.h>

#ifdef TVG_API
#undef TVG_API
#endif

#define TVG_VERSION_MAJOR 1
#define TVG_VERSION_MINOR 0
#define TVG_VERSION_MICRO 0

#ifndef TVG_STATIC
#ifdef _WIN32
#if TVG_BUILD
#define TVG_API __declspec(dllexport)
#else
#define TVG_API __declspec(dllimport)
#endif
#elif (defined(__SUNPRO_C) || defined(__SUNPRO_CC))
#define TVG_API __global
#else
#if (defined(__GNUC__) && __GNUC__ >= 4) || defined(__INTEL_COMPILER)
#define TVG_API __attribute__((visibility("default")))
#else
#define TVG_API
#endif
#endif
#else
#define TVG_API
#endif

#ifdef TVG_DEPRECATED
#undef TVG_DEPRECATED
#endif

#ifdef _WIN32
#define TVG_DEPRECATED __declspec(deprecated)
#elif __GNUC__ > 3 || (__GNUC__ == 3 && __GNUC_MINOR__ >= 1)
#define TVG_DEPRECATED __attribute__((__deprecated__))
#else
#define TVG_DEPRECATED
#endif

#ifdef __cplusplus
extern "C" {
#endif


typedef struct _Tvg_Canvas *Tvg_Canvas;

typedef struct _Tvg_Paint *Tvg_Paint;

typedef struct _Tvg_Gradient *Tvg_Gradient;

typedef struct _Tvg_Saver *Tvg_Saver;

typedef struct _Tvg_Animation *Tvg_Animation;

typedef struct _Tvg_Accessor *Tvg_Accessor;

typedef enum {
  TVG_RESULT_SUCCESS = 0,
  TVG_RESULT_INVALID_ARGUMENT,
  TVG_RESULT_INSUFFICIENT_CONDITION,
  TVG_RESULT_FAILED_ALLOCATION,
  TVG_RESULT_MEMORY_CORRUPTION,
  TVG_RESULT_NOT_SUPPORTED,
  TVG_RESULT_UNKNOWN = 255
} Tvg_Result;

typedef enum {
  TVG_COLORSPACE_ABGR8888 = 0,
  TVG_COLORSPACE_ARGB8888,
  TVG_COLORSPACE_ABGR8888S,
  TVG_COLORSPACE_ARGB8888S,
  TVG_COLORSPACE_UNKNOWN = 255,
} Tvg_Colorspace;

typedef enum {
  TVG_ENGINE_OPTION_NONE = 0,
  TVG_ENGINE_OPTION_DEFAULT = 1 << 0,
  TVG_ENGINE_OPTION_SMART_RENDER = 1 << 1
} Tvg_Engine_Option;

typedef enum {
  TVG_MASK_METHOD_NONE = 0,
  TVG_MASK_METHOD_ALPHA,
  TVG_MASK_METHOD_INVERSE_ALPHA,
  TVG_MASK_METHOD_LUMA,
  TVG_MASK_METHOD_INVERSE_LUMA,
  TVG_MASK_METHOD_ADD,
  TVG_MASK_METHOD_SUBTRACT,
  TVG_MASK_METHOD_INTERSECT,
  TVG_MASK_METHOD_DIFFERENCE,
  TVG_MASK_METHOD_LIGHTEN,
  TVG_MASK_METHOD_DARKEN
} Tvg_Mask_Method;

typedef enum {
  TVG_BLEND_METHOD_NORMAL = 0,
  TVG_BLEND_METHOD_MULTIPLY,
  TVG_BLEND_METHOD_SCREEN,
  TVG_BLEND_METHOD_OVERLAY,
  TVG_BLEND_METHOD_DARKEN,
  TVG_BLEND_METHOD_LIGHTEN,
  TVG_BLEND_METHOD_COLORDODGE,
  TVG_BLEND_METHOD_COLORBURN,
  TVG_BLEND_METHOD_HARDLIGHT,
  TVG_BLEND_METHOD_SOFTLIGHT,
  TVG_BLEND_METHOD_DIFFERENCE,
  TVG_BLEND_METHOD_EXCLUSION,
  TVG_BLEND_METHOD_HUE,
  TVG_BLEND_METHOD_SATURATION,
  TVG_BLEND_METHOD_COLOR,
  TVG_BLEND_METHOD_LUMINOSITY,
  TVG_BLEND_METHOD_ADD,
  TVG_BLEND_METHOD_COMPOSITION = 255
} Tvg_Blend_Method;

typedef enum {
  TVG_TYPE_UNDEF = 0,
  TVG_TYPE_SHAPE,
  TVG_TYPE_SCENE,
  TVG_TYPE_PICTURE,
  TVG_TYPE_TEXT,
  TVG_TYPE_LINEAR_GRAD = 10,
  TVG_TYPE_RADIAL_GRAD
} Tvg_Type;


typedef uint8_t Tvg_Path_Command;

enum {
  TVG_PATH_COMMAND_CLOSE = 0,
  TVG_PATH_COMMAND_MOVE_TO,
  TVG_PATH_COMMAND_LINE_TO,
  TVG_PATH_COMMAND_CUBIC_TO
};

typedef enum {
  TVG_STROKE_CAP_BUTT = 0,
  TVG_STROKE_CAP_ROUND,
  TVG_STROKE_CAP_SQUARE
} Tvg_Stroke_Cap;

typedef enum {
  TVG_STROKE_JOIN_MITER = 0,
  TVG_STROKE_JOIN_ROUND,
  TVG_STROKE_JOIN_BEVEL
} Tvg_Stroke_Join;

typedef enum {
  TVG_STROKE_FILL_PAD = 0,
  TVG_STROKE_FILL_REFLECT,
  TVG_STROKE_FILL_REPEAT
} Tvg_Stroke_Fill;

typedef enum {
  TVG_FILL_RULE_NON_ZERO = 0,
  TVG_FILL_RULE_EVEN_ODD
} Tvg_Fill_Rule;


typedef struct {
  float offset;
  uint8_t r;
  uint8_t g;
  uint8_t b;
  uint8_t a;
} Tvg_Color_Stop;


typedef enum {
  TVG_TEXT_WRAP_NONE = 0,
  TVG_TEXT_WRAP_CHARACTER,
  TVG_TEXT_WRAP_WORD,
  TVG_TEXT_WRAP_SMART,
  TVG_TEXT_WRAP_ELLIPSIS,
  TVG_TEXT_WRAP_HYPHENATION
} Tvg_Text_Wrap;

typedef struct {
  float x, y;
} Tvg_Point;

typedef struct {
  float e11, e12, e13;
  float e21, e22, e23;
  float e31, e32, e33;
} Tvg_Matrix;

typedef struct {
  float ascent;
  float descent;
  float linegap;
  float advance;
} Tvg_Text_Metrics;

typedef bool (*Tvg_Picture_Asset_Resolver)(Tvg_Paint paint, const char *src,
                                           void *data);


TVG_API Tvg_Result tvg_engine_init(unsigned threads);

TVG_API Tvg_Result tvg_engine_term(void);

TVG_API Tvg_Result tvg_engine_version(uint32_t *major, uint32_t *minor,
                                      uint32_t *micro, const char **version);



TVG_API Tvg_Canvas tvg_swcanvas_create(Tvg_Engine_Option op);

TVG_API Tvg_Result tvg_swcanvas_set_target(Tvg_Canvas canvas, uint32_t *buffer,
                                           uint32_t stride, uint32_t w,
                                           uint32_t h, Tvg_Colorspace cs);


TVG_API Tvg_Canvas tvg_glcanvas_create(Tvg_Engine_Option op);

TVG_API Tvg_Result tvg_glcanvas_set_target(Tvg_Canvas canvas, void *display,
                                           void *surface, void *context,
                                           int32_t id, uint32_t w, uint32_t h,
                                           Tvg_Colorspace cs);


TVG_API Tvg_Canvas tvg_wgcanvas_create(Tvg_Engine_Option op);

TVG_API Tvg_Result tvg_wgcanvas_set_target(Tvg_Canvas canvas, void *device,
                                           void *instance, void *target,
                                           uint32_t w, uint32_t h,
                                           Tvg_Colorspace cs, int type);

TVG_API Tvg_Result tvg_canvas_destroy(Tvg_Canvas canvas);

TVG_API Tvg_Result tvg_canvas_add(Tvg_Canvas canvas, Tvg_Paint paint);

TVG_API Tvg_Result tvg_canvas_insert(Tvg_Canvas canvas, Tvg_Paint target,
                                     Tvg_Paint at);

TVG_API Tvg_Result tvg_canvas_remove(Tvg_Canvas canvas, Tvg_Paint paint);

TVG_API Tvg_Result tvg_canvas_update(Tvg_Canvas canvas);

TVG_API Tvg_Result tvg_canvas_draw(Tvg_Canvas canvas, bool clear);

TVG_API Tvg_Result tvg_canvas_sync(Tvg_Canvas canvas);

TVG_API Tvg_Result tvg_canvas_set_viewport(Tvg_Canvas canvas, int32_t x,
                                           int32_t y, int32_t w, int32_t h);


TVG_API Tvg_Result tvg_paint_rel(Tvg_Paint paint);

TVG_API uint16_t tvg_paint_ref(Tvg_Paint paint);

TVG_API uint16_t tvg_paint_unref(Tvg_Paint paint, bool free);

TVG_API uint16_t tvg_paint_get_ref(const Tvg_Paint paint);

TVG_API Tvg_Result tvg_paint_set_visible(Tvg_Paint paint, bool visible);

TVG_API bool tvg_paint_get_visible(const Tvg_Paint paint);

TVG_API Tvg_Result tvg_paint_scale(Tvg_Paint paint, float factor);

TVG_API Tvg_Result tvg_paint_rotate(Tvg_Paint paint, float degree);

TVG_API Tvg_Result tvg_paint_translate(Tvg_Paint paint, float x, float y);

TVG_API Tvg_Result tvg_paint_set_transform(Tvg_Paint paint,
                                           const Tvg_Matrix *m);

TVG_API Tvg_Result tvg_paint_get_transform(Tvg_Paint paint, Tvg_Matrix *m);

TVG_API Tvg_Result tvg_paint_set_opacity(Tvg_Paint paint, uint8_t opacity);

TVG_API Tvg_Result tvg_paint_get_opacity(const Tvg_Paint paint,
                                         uint8_t *opacity);

TVG_API Tvg_Paint tvg_paint_duplicate(Tvg_Paint paint);

TVG_API bool tvg_paint_intersects(Tvg_Paint paint, int32_t x, int32_t y,
                                  int32_t w, int32_t h);

TVG_API Tvg_Result tvg_paint_get_aabb(Tvg_Paint paint, float *x, float *y,
                                      float *w, float *h);

TVG_API Tvg_Result tvg_paint_get_obb(Tvg_Paint paint, Tvg_Point *pt4);

TVG_API Tvg_Result tvg_paint_set_mask_method(Tvg_Paint paint, Tvg_Paint target,
                                             Tvg_Mask_Method method);

TVG_API Tvg_Result tvg_paint_get_mask_method(const Tvg_Paint paint,
                                             const Tvg_Paint target,
                                             Tvg_Mask_Method *method);

TVG_API Tvg_Result tvg_paint_set_clip(Tvg_Paint paint, Tvg_Paint clipper);

TVG_API Tvg_Paint tvg_paint_get_clip(const Tvg_Paint paint);

TVG_API Tvg_Paint tvg_paint_get_parent(const Tvg_Paint paint);

TVG_API Tvg_Result tvg_paint_get_type(const Tvg_Paint paint, Tvg_Type *type);

TVG_API Tvg_Result tvg_paint_set_blend_method(Tvg_Paint paint,
                                              Tvg_Blend_Method method);


TVG_API Tvg_Paint tvg_shape_new(void);

TVG_API Tvg_Result tvg_shape_reset(Tvg_Paint paint);

TVG_API Tvg_Result tvg_shape_move_to(Tvg_Paint paint, float x, float y);

TVG_API Tvg_Result tvg_shape_line_to(Tvg_Paint paint, float x, float y);

TVG_API Tvg_Result tvg_shape_cubic_to(Tvg_Paint paint, float cx1, float cy1,
                                      float cx2, float cy2, float x, float y);

TVG_API Tvg_Result tvg_shape_close(Tvg_Paint paint);

TVG_API Tvg_Result tvg_shape_append_rect(Tvg_Paint paint, float x, float y,
                                         float w, float h, float rx, float ry,
                                         bool cw);

TVG_API Tvg_Result tvg_shape_append_circle(Tvg_Paint paint, float cx, float cy,
                                           float rx, float ry, bool cw);

TVG_API Tvg_Result tvg_shape_append_path(Tvg_Paint paint,
                                         const Tvg_Path_Command *cmds,
                                         uint32_t cmdCnt, const Tvg_Point *pts,
                                         uint32_t ptsCnt);

TVG_API Tvg_Result tvg_shape_get_path(const Tvg_Paint paint,
                                      const Tvg_Path_Command **cmds,
                                      uint32_t *cmdsCnt, const Tvg_Point **pts,
                                      uint32_t *ptsCnt);

TVG_API Tvg_Result tvg_shape_set_stroke_width(Tvg_Paint paint, float width);

TVG_API Tvg_Result tvg_shape_get_stroke_width(const Tvg_Paint paint,
                                              float *width);

TVG_API Tvg_Result tvg_shape_set_stroke_color(Tvg_Paint paint, uint8_t r,
                                              uint8_t g, uint8_t b, uint8_t a);

TVG_API Tvg_Result tvg_shape_get_stroke_color(const Tvg_Paint paint, uint8_t *r,
                                              uint8_t *g, uint8_t *b,
                                              uint8_t *a);

TVG_API Tvg_Result tvg_shape_set_stroke_gradient(Tvg_Paint paint,
                                                 Tvg_Gradient grad);

TVG_API Tvg_Result tvg_shape_get_stroke_gradient(const Tvg_Paint paint,
                                                 Tvg_Gradient *grad);

TVG_API Tvg_Result tvg_shape_set_stroke_dash(Tvg_Paint paint,
                                             const float *dashPattern,
                                             uint32_t cnt, float offset);

TVG_API Tvg_Result tvg_shape_get_stroke_dash(const Tvg_Paint paint,
                                             const float **dashPattern,
                                             uint32_t *cnt, float *offset);

TVG_API Tvg_Result tvg_shape_set_stroke_cap(Tvg_Paint paint,
                                            Tvg_Stroke_Cap cap);

TVG_API Tvg_Result tvg_shape_get_stroke_cap(const Tvg_Paint paint,
                                            Tvg_Stroke_Cap *cap);

TVG_API Tvg_Result tvg_shape_set_stroke_join(Tvg_Paint paint,
                                             Tvg_Stroke_Join join);

TVG_API Tvg_Result tvg_shape_get_stroke_join(const Tvg_Paint paint,
                                             Tvg_Stroke_Join *join);

TVG_API Tvg_Result tvg_shape_set_stroke_miterlimit(Tvg_Paint paint,
                                                   float miterlimit);

TVG_API Tvg_Result tvg_shape_get_stroke_miterlimit(const Tvg_Paint paint,
                                                   float *miterlimit);

TVG_API Tvg_Result tvg_shape_set_trimpath(Tvg_Paint paint, float begin,
                                          float end, bool simultaneous);

TVG_API Tvg_Result tvg_shape_set_fill_color(Tvg_Paint paint, uint8_t r,
                                            uint8_t g, uint8_t b, uint8_t a);

TVG_API Tvg_Result tvg_shape_get_fill_color(const Tvg_Paint paint, uint8_t *r,
                                            uint8_t *g, uint8_t *b, uint8_t *a);

TVG_API Tvg_Result tvg_shape_set_fill_rule(Tvg_Paint paint, Tvg_Fill_Rule rule);

TVG_API Tvg_Result tvg_shape_get_fill_rule(const Tvg_Paint paint,
                                           Tvg_Fill_Rule *rule);

TVG_API Tvg_Result tvg_shape_set_paint_order(Tvg_Paint paint, bool strokeFirst);

TVG_API Tvg_Result tvg_shape_set_gradient(Tvg_Paint paint, Tvg_Gradient grad);

TVG_API Tvg_Result tvg_shape_get_gradient(const Tvg_Paint paint,
                                          Tvg_Gradient *grad);


TVG_API Tvg_Gradient tvg_linear_gradient_new(void);

TVG_API Tvg_Gradient tvg_radial_gradient_new(void);

TVG_API Tvg_Result tvg_linear_gradient_set(Tvg_Gradient grad, float x1,
                                           float y1, float x2, float y2);

TVG_API Tvg_Result tvg_linear_gradient_get(Tvg_Gradient grad, float *x1,
                                           float *y1, float *x2, float *y2);

TVG_API Tvg_Result tvg_radial_gradient_set(Tvg_Gradient grad, float cx,
                                           float cy, float r, float fx,
                                           float fy, float fr);

TVG_API Tvg_Result tvg_radial_gradient_get(Tvg_Gradient grad, float *cx,
                                           float *cy, float *r, float *fx,
                                           float *fy, float *fr);

TVG_API Tvg_Result tvg_gradient_set_color_stops(
    Tvg_Gradient grad, const Tvg_Color_Stop *color_stop, uint32_t cnt);

TVG_API Tvg_Result tvg_gradient_get_color_stops(
    const Tvg_Gradient grad, const Tvg_Color_Stop **color_stop, uint32_t *cnt);

TVG_API Tvg_Result tvg_gradient_set_spread(Tvg_Gradient grad,
                                           const Tvg_Stroke_Fill spread);

TVG_API Tvg_Result tvg_gradient_get_spread(const Tvg_Gradient grad,
                                           Tvg_Stroke_Fill *spread);

TVG_API Tvg_Result tvg_gradient_set_transform(Tvg_Gradient grad,
                                              const Tvg_Matrix *m);

TVG_API Tvg_Result tvg_gradient_get_transform(const Tvg_Gradient grad,
                                              Tvg_Matrix *m);

TVG_API Tvg_Result tvg_gradient_get_type(const Tvg_Gradient grad,
                                         Tvg_Type *type);

TVG_API Tvg_Gradient tvg_gradient_duplicate(Tvg_Gradient grad);

TVG_API Tvg_Result tvg_gradient_del(Tvg_Gradient grad);


TVG_API Tvg_Paint tvg_picture_new(void);

TVG_API Tvg_Result tvg_picture_load(Tvg_Paint picture, const char *path);

TVG_API Tvg_Result tvg_picture_load_raw(Tvg_Paint picture, const uint32_t *data,
                                        uint32_t w, uint32_t h,
                                        Tvg_Colorspace cs, bool copy);

TVG_API Tvg_Result tvg_picture_load_data(Tvg_Paint picture, const char *data,
                                         uint32_t size, const char *mimetype,
                                         const char *rpath, bool copy);

TVG_API Tvg_Result tvg_picture_set_asset_resolver(
    Tvg_Paint picture, Tvg_Picture_Asset_Resolver resolver, void *data);

TVG_API Tvg_Result tvg_picture_set_size(Tvg_Paint picture, float w, float h);

TVG_API Tvg_Result tvg_picture_get_size(const Tvg_Paint picture, float *w,
                                        float *h);

TVG_API Tvg_Result tvg_picture_set_origin(Tvg_Paint picture, float x, float y);

TVG_API Tvg_Result tvg_picture_get_origin(const Tvg_Paint picture, float *x,
                                          float *y);

TVG_API const Tvg_Paint tvg_picture_get_paint(Tvg_Paint picture, uint32_t id);


TVG_API Tvg_Paint tvg_scene_new(void);

TVG_API Tvg_Result tvg_scene_add(Tvg_Paint scene, Tvg_Paint paint);

TVG_API Tvg_Result tvg_scene_insert(Tvg_Paint scene, Tvg_Paint target,
                                    Tvg_Paint at);

TVG_API Tvg_Result tvg_scene_remove(Tvg_Paint scene, Tvg_Paint paint);

TVG_API Tvg_Result tvg_scene_clear_effects(Tvg_Paint scene);

TVG_API Tvg_Result tvg_scene_add_effect_gaussian_blur(Tvg_Paint scene,
                                                      double sigma,
                                                      int direction, int border,
                                                      int quality);

TVG_API Tvg_Result tvg_scene_add_effect_drop_shadow(Tvg_Paint scene, int r,
                                                    int g, int b, int a,
                                                    double angle,
                                                    double distance,
                                                    double sigma, int quality);

TVG_API Tvg_Result tvg_scene_add_effect_fill(Tvg_Paint scene, int r, int g,
                                             int b, int a);

TVG_API Tvg_Result tvg_scene_add_effect_tint(Tvg_Paint scene, int black_r,
                                             int black_g, int black_b,
                                             int white_r, int white_g,
                                             int white_b, double intensity);

TVG_API Tvg_Result tvg_scene_add_effect_tritone(Tvg_Paint scene, int shadow_r,
                                                int shadow_g, int shadow_b,
                                                int midtone_r, int midtone_g,
                                                int midtone_b, int highlight_r,
                                                int highlight_g,
                                                int highlight_b, int blend);


TVG_API Tvg_Paint tvg_text_new(void);

TVG_API Tvg_Result tvg_text_set_font(Tvg_Paint text, const char *name);

TVG_API Tvg_Result tvg_text_set_size(Tvg_Paint text, float size);

TVG_API Tvg_Result tvg_text_set_text(Tvg_Paint text, const char *utf8);

TVG_API Tvg_Result tvg_text_align(Tvg_Paint text, float x, float y);

TVG_API Tvg_Result tvg_text_layout(Tvg_Paint text, float w, float h);

TVG_API Tvg_Result tvg_text_wrap_mode(Tvg_Paint text, Tvg_Text_Wrap mode);

TVG_API Tvg_Result tvg_text_spacing(Tvg_Paint text, float letter, float line);

TVG_API Tvg_Result tvg_text_set_italic(Tvg_Paint text, float shear);

TVG_API Tvg_Result tvg_text_set_outline(Tvg_Paint text, float width, uint8_t r,
                                        uint8_t g, uint8_t b);

TVG_API Tvg_Result tvg_text_set_color(Tvg_Paint text, uint8_t r, uint8_t g,
                                      uint8_t b);

TVG_API Tvg_Result tvg_text_set_gradient(Tvg_Paint text, Tvg_Gradient gradient);

TVG_API Tvg_Result tvg_text_get_metrics(const Tvg_Paint text,
                                        Tvg_Text_Metrics *metrics);

TVG_API Tvg_Result tvg_font_load(const char *path);

TVG_API Tvg_Result tvg_font_load_data(const char *name, const char *data,
                                      uint32_t size, const char *mimetype,
                                      bool copy);

TVG_API Tvg_Result tvg_font_unload(const char *path);


TVG_API Tvg_Saver tvg_saver_new(void);

TVG_API Tvg_Result tvg_saver_save_paint(Tvg_Saver saver, Tvg_Paint paint,
                                        const char *path, uint32_t quality);

TVG_API Tvg_Result tvg_saver_save_animation(Tvg_Saver saver,
                                            Tvg_Animation animation,
                                            const char *path, uint32_t quality,
                                            uint32_t fps);

TVG_API Tvg_Result tvg_saver_sync(Tvg_Saver saver);

TVG_API Tvg_Result tvg_saver_del(Tvg_Saver saver);


TVG_API Tvg_Animation tvg_animation_new(void);

TVG_API Tvg_Result tvg_animation_set_frame(Tvg_Animation animation, float no);

TVG_API Tvg_Paint tvg_animation_get_picture(Tvg_Animation animation);

TVG_API Tvg_Result tvg_animation_get_frame(Tvg_Animation animation, float *no);

TVG_API Tvg_Result tvg_animation_get_total_frame(Tvg_Animation animation,
                                                 float *cnt);

TVG_API Tvg_Result tvg_animation_get_duration(Tvg_Animation animation,
                                              float *duration);

TVG_API Tvg_Result tvg_animation_set_segment(Tvg_Animation animation,
                                             float begin, float end);

TVG_API Tvg_Result tvg_animation_get_segment(Tvg_Animation animation,
                                             float *begin, float *end);

TVG_API Tvg_Result tvg_animation_del(Tvg_Animation animation);


TVG_API Tvg_Accessor tvg_accessor_new(void);

TVG_API Tvg_Result tvg_accessor_del(Tvg_Accessor accessor);

TVG_API Tvg_Result tvg_accessor_set(Tvg_Accessor accessor, Tvg_Paint paint,
                                    bool (*func)(Tvg_Paint paint, void *data),
                                    void *data);

TVG_API uint32_t tvg_accessor_generate_id(const char *name);


TVG_API Tvg_Animation tvg_lottie_animation_new(void);

TVG_API uint32_t tvg_lottie_animation_gen_slot(Tvg_Animation animation,
                                               const char *slot);

TVG_API Tvg_Result tvg_lottie_animation_apply_slot(Tvg_Animation animation,
                                                   uint32_t id);

TVG_API Tvg_Result tvg_lottie_animation_del_slot(Tvg_Animation animation,
                                                 uint32_t id);

TVG_API Tvg_Result tvg_lottie_animation_set_marker(Tvg_Animation animation,
                                                   const char *marker);

TVG_API Tvg_Result tvg_lottie_animation_get_markers_cnt(Tvg_Animation animation,
                                                        uint32_t *cnt);

TVG_API Tvg_Result tvg_lottie_animation_get_marker(Tvg_Animation animation,
                                                   uint32_t idx,
                                                   const char **name);

TVG_API Tvg_Result tvg_lottie_animation_tween(Tvg_Animation animation,
                                              float from, float to,
                                              float progress);

TVG_API Tvg_Result tvg_lottie_animation_assign(Tvg_Animation animation,
                                               const char *layer, uint32_t ix,
                                               const char *var, float val);

TVG_API Tvg_Result tvg_lottie_animation_set_quality(Tvg_Animation animation,
                                                    uint8_t value);

#ifdef __cplusplus
}
#endif

#endif
