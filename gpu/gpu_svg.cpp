#include "gpu_svg.h"

#include "../lunasvg/include/lunasvg.h"

#include <cctype>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <memory>
#include <vector>
struct gpu_svg_document {
    std::unique_ptr<lunasvg::Document> document;
    float width;
    float height;
    char* source;
};

static const float SQUIRCLE_KX[] = {1.498f, 3.381f, 7.456f, 12.630f, 17.368f, 21.770f, 30.573f};
static const float SQUIRCLE_KY[] = {0.800f, 3.600f, 7.370f, 12.544f, 16.619f, 18.502f, 20.000f};

static const char* skip_spaces(const char* p)
{
    while(p && *p && std::isspace(static_cast<unsigned char>(*p)))
        ++p;
    return p;
}

static const char* find_attr_value(const char* tag, const char* attr)
{
    if(tag == nullptr || attr == nullptr)
        return nullptr;

    const char* tag_end = std::strchr(tag, '>');
    if(tag_end == nullptr)
        tag_end = tag + std::strlen(tag);

    size_t attr_len = std::strlen(attr);
    const char* p = tag;
    while((p = std::strstr(p, attr)) != nullptr) {
        if(p >= tag_end)
            break;
        if(p != tag) {
            char prev = p[-1];
            if(std::isalnum(static_cast<unsigned char>(prev)) || prev == '_' || prev == '-') {
                p += attr_len;
                continue;
            }
        }
        const char* q = skip_spaces(p + attr_len);
        if(q == nullptr || q >= tag_end || *q != '=') {
            p += attr_len;
            continue;
        }
        q = skip_spaces(q + 1);
        if(q == nullptr || q >= tag_end || (*q != '"' && *q != '\''))
            return nullptr;
        return q + 1;
    }
    return nullptr;
}

static float parse_attr_float(const char* tag, const char* attr, float fallback)
{
    const char* value = find_attr_value(tag, attr);
    if(value == nullptr)
        return fallback;
    if(!((*value >= '0' && *value <= '9') || *value == '-' || *value == '+' || *value == '.'))
        return fallback;

    int sign = 1;
    if(*value == '-') {
        sign = -1;
        ++value;
    } else if(*value == '+') {
        ++value;
    }

    float result = 0.0f;
    while(*value >= '0' && *value <= '9') {
        result = result * 10.0f + static_cast<float>(*value - '0');
        ++value;
    }

    if(*value == '.') {
        ++value;
        float place = 0.1f;
        while(*value >= '0' && *value <= '9') {
            result += static_cast<float>(*value - '0') * place;
            place *= 0.1f;
            ++value;
        }
    }

    return result * static_cast<float>(sign);
}

static bool find_marker_bounds(const char* source, const char* marker, const char*& start, const char*& end)
{
    if(source == nullptr || marker == nullptr)
        return false;
    start = std::strstr(source, marker);
    if(start == nullptr)
        return false;
    end = std::strstr(start, "-->");
    return end != nullptr;
}

static unsigned parse_hex_pair(char hi, char lo)
{
    auto hexval = [](char c) -> unsigned {
        if(c >= '0' && c <= '9') return static_cast<unsigned>(c - '0');
        if(c >= 'a' && c <= 'f') return static_cast<unsigned>(c - 'a' + 10);
        if(c >= 'A' && c <= 'F') return static_cast<unsigned>(c - 'A' + 10);
        return 0;
    };
    return (hexval(hi) << 4) | hexval(lo);
}

static bool parse_paint_rgba(const char* tag, const char* attr, const char* opacity_attr,
                             unsigned char& r, unsigned char& g,
                             unsigned char& b, unsigned char& a)
{
    const char* fill = find_attr_value(tag, attr);
    if(fill == nullptr)
        return false;
    if(std::strncmp(fill, "none", 4) == 0 || std::strncmp(fill, "transparent", 11) == 0)
        return false;
    if(*fill != '#')
        return false;

    size_t len = 0;
    while(fill[len] && fill[len] != '"' && fill[len] != '\'' && !std::isspace(static_cast<unsigned char>(fill[len])))
        ++len;

    if(len == 9) {
        r = static_cast<unsigned char>(parse_hex_pair(fill[1], fill[2]));
        g = static_cast<unsigned char>(parse_hex_pair(fill[3], fill[4]));
        b = static_cast<unsigned char>(parse_hex_pair(fill[5], fill[6]));
        a = static_cast<unsigned char>(parse_hex_pair(fill[7], fill[8]));
    } else if(len == 7) {
        r = static_cast<unsigned char>(parse_hex_pair(fill[1], fill[2]));
        g = static_cast<unsigned char>(parse_hex_pair(fill[3], fill[4]));
        b = static_cast<unsigned char>(parse_hex_pair(fill[5], fill[6]));
    } else if(len == 4) {
        unsigned rr = parse_hex_pair(fill[1], fill[1]);
        unsigned gg = parse_hex_pair(fill[2], fill[2]);
        unsigned bb = parse_hex_pair(fill[3], fill[3]);
        r = static_cast<unsigned char>(rr);
        g = static_cast<unsigned char>(gg);
        b = static_cast<unsigned char>(bb);
    } else {
        return false;
    }

    float opacity = parse_attr_float(tag, "opacity", 1.0f);
    float local_opacity = parse_attr_float(tag, opacity_attr, 1.0f);
    opacity *= local_opacity;
    if(opacity < 0.0f) opacity = 0.0f;
    if(opacity > 1.0f) opacity = 1.0f;
    unsigned alpha_base = a;
    if(len != 9)
        alpha_base = 255u;
    a = static_cast<unsigned char>((alpha_base * static_cast<unsigned>(opacity * 255.0f + 0.5f) + 127u) / 255u);
    return a != 0;
}

static void blend_pixel(unsigned char* px,
                        unsigned char sr, unsigned char sg, unsigned char sb, unsigned char sa)
{
    if(sa == 0)
        return;
    if(px[3] == 0 || sa == 255) {
        px[0] = sr;
        px[1] = sg;
        px[2] = sb;
        px[3] = sa;
        return;
    }

    unsigned dst_a = px[3];
    unsigned inv_sa = 255u - sa;
    unsigned out_a = sa + ((dst_a * inv_sa + 127u) / 255u);
    if(out_a == 0) {
        px[0] = 0;
        px[1] = 0;
        px[2] = 0;
        px[3] = 0;
        return;
    }

    unsigned src_pr = static_cast<unsigned>(sr) * sa;
    unsigned src_pg = static_cast<unsigned>(sg) * sa;
    unsigned src_pb = static_cast<unsigned>(sb) * sa;
    unsigned dst_pr = static_cast<unsigned>(px[0]) * dst_a;
    unsigned dst_pg = static_cast<unsigned>(px[1]) * dst_a;
    unsigned dst_pb = static_cast<unsigned>(px[2]) * dst_a;
    unsigned out_pr = src_pr + ((dst_pr * inv_sa + 127u) / 255u);
    unsigned out_pg = src_pg + ((dst_pg * inv_sa + 127u) / 255u);
    unsigned out_pb = src_pb + ((dst_pb * inv_sa + 127u) / 255u);

    px[0] = static_cast<unsigned char>((out_pr + out_a / 2u) / out_a);
    px[1] = static_cast<unsigned char>((out_pg + out_a / 2u) / out_a);
    px[2] = static_cast<unsigned char>((out_pb + out_a / 2u) / out_a);
    px[3] = static_cast<unsigned char>(out_a > 255u ? 255u : out_a);
}

static void blend_pixel_argb_premul(uint32_t* px,
                                    unsigned char sr, unsigned char sg, unsigned char sb, unsigned char sa)
{
    if(sa == 0)
        return;

    uint32_t dst = *px;
    uint32_t dst_a = (dst >> 24) & 0xFFu;
    uint32_t dst_r = (dst >> 16) & 0xFFu;
    uint32_t dst_g = (dst >> 8) & 0xFFu;
    uint32_t dst_b = dst & 0xFFu;
    uint32_t inv_sa = 255u - sa;

    uint32_t src_r = (static_cast<uint32_t>(sr) * sa + 127u) / 255u;
    uint32_t src_g = (static_cast<uint32_t>(sg) * sa + 127u) / 255u;
    uint32_t src_b = (static_cast<uint32_t>(sb) * sa + 127u) / 255u;
    uint32_t out_a = sa + ((dst_a * inv_sa + 127u) / 255u);
    uint32_t out_r = src_r + ((dst_r * inv_sa + 127u) / 255u);
    uint32_t out_g = src_g + ((dst_g * inv_sa + 127u) / 255u);
    uint32_t out_b = src_b + ((dst_b * inv_sa + 127u) / 255u);

    if(out_a > 255u) out_a = 255u;
    if(out_r > 255u) out_r = 255u;
    if(out_g > 255u) out_g = 255u;
    if(out_b > 255u) out_b = 255u;
    *px = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
}

static bool point_in_rounded_rect_sample(float x, float y, int w, int h, int rx, int ry)
{
    if(w <= 0 || h <= 0)
        return false;
    if(rx <= 0 || ry <= 0)
        return x >= 0.0f && y >= 0.0f && x < static_cast<float>(w) && y < static_cast<float>(h);

    if(x < 0.0f || y < 0.0f || x >= static_cast<float>(w) || y >= static_cast<float>(h))
        return false;

    if((x >= static_cast<float>(rx) && x < static_cast<float>(w - rx)) ||
       (y >= static_cast<float>(ry) && y < static_cast<float>(h - ry)))
        return true;

    float cx = (x < static_cast<float>(rx)) ? static_cast<float>(rx) : static_cast<float>(w - rx - 1);
    float cy = (y < static_cast<float>(ry)) ? static_cast<float>(ry) : static_cast<float>(h - ry - 1);
    float dx = x - cx;
    float dy = y - cy;
    float ndx = dx / static_cast<float>(rx);
    float ndy = dy / static_cast<float>(ry);
    return (ndx * ndx + ndy * ndy) <= 1.0f;
}

static unsigned char rounded_rect_coverage(int x, int y, int w, int h, int rx, int ry)
{
    static const float OFFSETS[4][2] = {
        {0.25f, 0.25f},
        {0.75f, 0.25f},
        {0.25f, 0.75f},
        {0.75f, 0.75f},
    };
    int hits = 0;
    for(int i = 0; i < 4; ++i) {
        if(point_in_rounded_rect_sample(static_cast<float>(x) + OFFSETS[i][0],
                                        static_cast<float>(y) + OFFSETS[i][1],
                                        w, h, rx, ry)) {
            ++hits;
        }
    }
    return static_cast<unsigned char>((hits * 255) / 4);
}

static void resolve_squircle_geometry(int w, int h, float radius, float& edge_x, float& edge_y, float& curve_scale)
{
    float fw = static_cast<float>(w);
    float fh = static_cast<float>(h);
    float min_edge = (fw < fh) ? fw : fh;
    float max_possible_radius = min_edge / 2.0f;

    if(radius == -1.0f) {
        float s = (fh >= 46.0f) ? 1.15f : 1.0f;
        curve_scale = s;
        edge_x = SQUIRCLE_KX[6] * s;
        edge_y = SQUIRCLE_KY[6] * s;
        if(edge_x > fw / 2.0f) edge_x = fw / 2.0f;
        if(edge_y > fh / 2.0f) edge_y = fh / 2.0f;
        return;
    }

    float radius_px;
    if(radius >= 1000.0f) {
        float radius_pct = radius - 1000.0f;
        if(radius_pct > 100.0f) radius_pct = 100.0f;
        if(radius_pct < 0.0f) radius_pct = 0.0f;
        radius_px = (max_possible_radius * radius_pct) / 100.0f;
    } else {
        radius_px = radius;
    }
    if(radius_px > max_possible_radius) radius_px = max_possible_radius;

    float s = (fh >= 46.0f) ? 1.15f : 1.0f;
    float default_radius = 12.0f * s;
    float scale = (default_radius > 0.0f) ? (radius_px / default_radius) : 1.0f;
    float max_scale_x = (fw / 2.0f) / (SQUIRCLE_KX[6] * s);
    float max_scale_y = (fh / 2.0f) / (SQUIRCLE_KY[6] * s);
    float max_scale = (max_scale_x < max_scale_y) ? max_scale_x : max_scale_y;
    if(scale > max_scale) scale = max_scale;
    if(scale < 0.0f) scale = 0.0f;

    curve_scale = s * scale;
    edge_x = SQUIRCLE_KX[6] * curve_scale;
    edge_y = SQUIRCLE_KY[6] * curve_scale;
    if(edge_x > fw / 2.0f) edge_x = fw / 2.0f;
    if(edge_y > fh / 2.0f) edge_y = fh / 2.0f;
}

struct SquirclePoint { float x; float y; };

static SquirclePoint cubic_point(float t, const SquirclePoint& p0, const SquirclePoint& p1,
                                 const SquirclePoint& p2, const SquirclePoint& p3)
{
    float mt = 1.0f - t;
    float mt2 = mt * mt;
    float t2 = t * t;
    return {
        mt2 * mt * p0.x + 3.0f * mt2 * t * p1.x + 3.0f * mt * t2 * p2.x + t2 * t * p3.x,
        mt2 * mt * p0.y + 3.0f * mt2 * t * p1.y + 3.0f * mt * t2 * p2.y + t2 * t * p3.y
    };
}

static void append_cubic(std::vector<SquirclePoint>& pts, const SquirclePoint& p0, const SquirclePoint& p1,
                         const SquirclePoint& p2, const SquirclePoint& p3)
{
    for(int i = 1; i <= 8; ++i) {
        float t = static_cast<float>(i) / 8.0f;
        pts.push_back(cubic_point(t, p0, p1, p2, p3));
    }
}

static void build_squircle_polygon(float x, float y, int w, int h, float radius, std::vector<SquirclePoint>& pts)
{
    pts.clear();
    float fw = static_cast<float>(w);
    float fh = static_cast<float>(h);
    float edge_x, edge_y, s;
    resolve_squircle_geometry(w, h, radius, edge_x, edge_y, s);

    SquirclePoint p0{ x + fw, y + fh / 2.0f };
    pts.push_back(p0);
    pts.push_back({ x + fw, y + fh - edge_y });
    append_cubic(pts, { x + fw, y + fh - edge_y },
                 { x + fw, y + fh - edge_y + SQUIRCLE_KY[0] * s },
                 { x + fw, y + fh - edge_y + SQUIRCLE_KY[1] * s },
                 { x + fw - SQUIRCLE_KX[0] * s, y + fh - edge_y + SQUIRCLE_KY[2] * s });
    append_cubic(pts, { x + fw - SQUIRCLE_KX[0] * s, y + fh - edge_y + SQUIRCLE_KY[2] * s },
                 { x + fw - SQUIRCLE_KX[1] * s, y + fh - edge_y + SQUIRCLE_KY[3] * s },
                 { x + fw - SQUIRCLE_KX[2] * s, y + fh - edge_y + SQUIRCLE_KY[4] * s },
                 { x + fw - SQUIRCLE_KX[3] * s, y + fh - edge_y + SQUIRCLE_KY[5] * s });
    append_cubic(pts, { x + fw - SQUIRCLE_KX[3] * s, y + fh - edge_y + SQUIRCLE_KY[5] * s },
                 { x + fw - SQUIRCLE_KX[4] * s, y + fh },
                 { x + fw - SQUIRCLE_KX[5] * s, y + fh },
                 { x + fw - edge_x, y + fh });
    pts.push_back({ x + edge_x, y + fh });
    append_cubic(pts, { x + edge_x, y + fh },
                 { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[5]) * s, y + fh },
                 { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[4]) * s, y + fh },
                 { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + fh - edge_y + SQUIRCLE_KY[5] * s });
    append_cubic(pts, { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + fh - edge_y + SQUIRCLE_KY[5] * s },
                 { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[2]) * s, y + fh - edge_y + SQUIRCLE_KY[4] * s },
                 { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[1]) * s, y + fh - edge_y + SQUIRCLE_KY[3] * s },
                 { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + fh - edge_y + SQUIRCLE_KY[2] * s });
    append_cubic(pts, { x + edge_x - (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + fh - edge_y + SQUIRCLE_KY[2] * s },
                 { x, y + fh - edge_y + SQUIRCLE_KY[1] * s },
                 { x, y + fh - edge_y + SQUIRCLE_KY[0] * s },
                 { x, y + fh - edge_y });
    pts.push_back({ x, y + edge_y });
    append_cubic(pts, { x, y + edge_y },
                 { x, y + edge_y - SQUIRCLE_KY[0] * s },
                 { x, y + edge_y - SQUIRCLE_KY[1] * s },
                 { x + SQUIRCLE_KX[0] * s, y + edge_y - SQUIRCLE_KY[2] * s });
    append_cubic(pts, { x + SQUIRCLE_KX[0] * s, y + edge_y - SQUIRCLE_KY[2] * s },
                 { x + SQUIRCLE_KX[1] * s, y + edge_y - SQUIRCLE_KY[3] * s },
                 { x + SQUIRCLE_KX[2] * s, y + edge_y - SQUIRCLE_KY[4] * s },
                 { x + SQUIRCLE_KX[3] * s, y + edge_y - SQUIRCLE_KY[5] * s });
    append_cubic(pts, { x + SQUIRCLE_KX[3] * s, y + edge_y - SQUIRCLE_KY[5] * s },
                 { x + SQUIRCLE_KX[4] * s, y },
                 { x + SQUIRCLE_KX[5] * s, y },
                 { x + edge_x, y });
    pts.push_back({ x + fw - edge_x, y });
    append_cubic(pts, { x + fw - edge_x, y },
                 { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[5]) * s, y },
                 { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[4]) * s, y },
                 { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + edge_y - SQUIRCLE_KY[5] * s });
    append_cubic(pts, { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[3]) * s, y + edge_y - SQUIRCLE_KY[5] * s },
                 { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[2]) * s, y + edge_y - SQUIRCLE_KY[4] * s },
                 { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[1]) * s, y + edge_y - SQUIRCLE_KY[3] * s },
                 { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + edge_y - SQUIRCLE_KY[2] * s });
    append_cubic(pts, { x + fw - edge_x + (SQUIRCLE_KX[6] - SQUIRCLE_KX[0]) * s, y + edge_y - SQUIRCLE_KY[2] * s },
                 { x + fw, y + edge_y - SQUIRCLE_KY[1] * s },
                 { x + fw, y + edge_y - SQUIRCLE_KY[0] * s },
                 { x + fw, y + edge_y });
}

static bool point_in_polygon(float x, float y, const std::vector<SquirclePoint>& pts)
{
    bool inside = false;
    size_t n = pts.size();
    if(n < 3)
        return false;
    for(size_t i = 0, j = n - 1; i < n; j = i++) {
        const SquirclePoint& a = pts[i];
        const SquirclePoint& b = pts[j];
        bool intersect = ((a.y > y) != (b.y > y)) &&
                         (x < (b.x - a.x) * (y - a.y) / ((b.y - a.y) == 0.0f ? 1e-6f : (b.y - a.y)) + a.x);
        if(intersect)
            inside = !inside;
    }
    return inside;
}

static unsigned char squircle_coverage(int px, int py, const std::vector<SquirclePoint>& pts)
{
    static const float OFFSETS[4][2] = {{0.25f,0.25f},{0.75f,0.25f},{0.25f,0.75f},{0.75f,0.75f}};
    int hits = 0;
    for(int i = 0; i < 4; ++i) {
        if(point_in_polygon(static_cast<float>(px) + OFFSETS[i][0], static_cast<float>(py) + OFFSETS[i][1], pts))
            ++hits;
    }
    return static_cast<unsigned char>((hits * 255) / 4);
}

static void fill_rounded_rect_fallback(unsigned char* out_rgba, int buf_w, int buf_h, int stride,
                                       int x0, int y0, int w, int h, int rx, int ry,
                                       unsigned char r, unsigned char g, unsigned char b, unsigned char a)
{
    if(a == 0 || w <= 0 || h <= 0)
        return;

    int orig_x0 = x0;
    int orig_y0 = y0;
    int x1 = x0 + w;
    int y1 = y0 + h;
    if(x0 < 0) x0 = 0;
    if(y0 < 0) y0 = 0;
    if(x1 > buf_w) x1 = buf_w;
    if(y1 > buf_h) y1 = buf_h;
    if(x0 >= x1 || y0 >= y1)
        return;

    for(int y = y0; y < y1; ++y) {
        unsigned char* row = out_rgba + static_cast<size_t>(y) * static_cast<size_t>(stride);
        for(int x = x0; x < x1; ++x) {
            unsigned char coverage = rounded_rect_coverage(x - orig_x0, y - orig_y0, w, h, rx, ry);
            if(coverage == 0)
                continue;
            unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(a) * coverage + 127u) / 255u);
            blend_pixel(row + x * 4, r, g, b, final_a);
        }
    }
}

static void stroke_rounded_rect_fallback(unsigned char* out_rgba, int buf_w, int buf_h, int stride,
                                         int x0, int y0, int w, int h, int rx, int ry, int stroke_w,
                                         unsigned char r, unsigned char g, unsigned char b, unsigned char a)
{
    if(a == 0 || stroke_w <= 0 || w <= 0 || h <= 0)
        return;

    int inner_w = w - stroke_w * 2;
    int inner_h = h - stroke_w * 2;
    int inner_rx = rx - stroke_w;
    int inner_ry = ry - stroke_w;
    if(inner_rx < 0) inner_rx = 0;
    if(inner_ry < 0) inner_ry = 0;

    int orig_x0 = x0;
    int orig_y0 = y0;
    int x1 = x0 + w;
    int y1 = y0 + h;
    if(x0 < 0) x0 = 0;
    if(y0 < 0) y0 = 0;
    if(x1 > buf_w) x1 = buf_w;
    if(y1 > buf_h) y1 = buf_h;
    if(x0 >= x1 || y0 >= y1)
        return;

    for(int y = y0; y < y1; ++y) {
        unsigned char* row = out_rgba + static_cast<size_t>(y) * static_cast<size_t>(stride);
        for(int x = x0; x < x1; ++x) {
            int lx = x - orig_x0;
            int ly = y - orig_y0;
            unsigned char outer = rounded_rect_coverage(lx, ly, w, h, rx, ry);
            if(outer == 0)
                continue;
            if(inner_w > 0 && inner_h > 0 &&
               rounded_rect_coverage(lx - stroke_w, ly - stroke_w, inner_w, inner_h, inner_rx, inner_ry) == 255)
                continue;
            unsigned char inner = 0;
            if(inner_w > 0 && inner_h > 0)
                inner = rounded_rect_coverage(lx - stroke_w, ly - stroke_w, inner_w, inner_h, inner_rx, inner_ry);
            unsigned char edge = static_cast<unsigned char>(outer > inner ? outer - inner : 0);
            if(edge == 0)
                continue;
            unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(a) * edge + 127u) / 255u);
            blend_pixel(row + x * 4, r, g, b, final_a);
        }
    }
}

static void fill_rounded_rect_fallback_premul(unsigned char* out_argb, int buf_w, int buf_h, int stride,
                                              int x0, int y0, int w, int h, int rx, int ry,
                                              unsigned char r, unsigned char g, unsigned char b, unsigned char a)
{
    if(a == 0 || w <= 0 || h <= 0)
        return;

    int orig_x0 = x0;
    int orig_y0 = y0;
    int x1 = x0 + w;
    int y1 = y0 + h;
    if(x0 < 0) x0 = 0;
    if(y0 < 0) y0 = 0;
    if(x1 > buf_w) x1 = buf_w;
    if(y1 > buf_h) y1 = buf_h;
    if(x0 >= x1 || y0 >= y1)
        return;

    for(int y = y0; y < y1; ++y) {
        uint32_t* row = reinterpret_cast<uint32_t*>(out_argb + static_cast<size_t>(y) * static_cast<size_t>(stride));
        for(int x = x0; x < x1; ++x) {
            unsigned char coverage = rounded_rect_coverage(x - orig_x0, y - orig_y0, w, h, rx, ry);
            if(coverage == 0)
                continue;
            unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(a) * coverage + 127u) / 255u);
            blend_pixel_argb_premul(&row[x], r, g, b, final_a);
        }
    }
}

static void stroke_rounded_rect_fallback_premul(unsigned char* out_argb, int buf_w, int buf_h, int stride,
                                                int x0, int y0, int w, int h, int rx, int ry, int stroke_w,
                                                unsigned char r, unsigned char g, unsigned char b, unsigned char a)
{
    if(a == 0 || stroke_w <= 0 || w <= 0 || h <= 0)
        return;

    int inner_w = w - stroke_w * 2;
    int inner_h = h - stroke_w * 2;
    int inner_rx = rx - stroke_w;
    int inner_ry = ry - stroke_w;
    if(inner_rx < 0) inner_rx = 0;
    if(inner_ry < 0) inner_ry = 0;

    int orig_x0 = x0;
    int orig_y0 = y0;
    int x1 = x0 + w;
    int y1 = y0 + h;
    if(x0 < 0) x0 = 0;
    if(y0 < 0) y0 = 0;
    if(x1 > buf_w) x1 = buf_w;
    if(y1 > buf_h) y1 = buf_h;
    if(x0 >= x1 || y0 >= y1)
        return;

    for(int y = y0; y < y1; ++y) {
        uint32_t* row = reinterpret_cast<uint32_t*>(out_argb + static_cast<size_t>(y) * static_cast<size_t>(stride));
        for(int x = x0; x < x1; ++x) {
            int lx = x - orig_x0;
            int ly = y - orig_y0;
            unsigned char outer = rounded_rect_coverage(lx, ly, w, h, rx, ry);
            if(outer == 0)
                continue;
            unsigned char inner = 0;
            if(inner_w > 0 && inner_h > 0)
                inner = rounded_rect_coverage(lx - stroke_w, ly - stroke_w, inner_w, inner_h, inner_rx, inner_ry);
            unsigned char edge = static_cast<unsigned char>(outer > inner ? outer - inner : 0);
            if(edge == 0)
                continue;
            unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(a) * edge + 127u) / 255u);
            blend_pixel_argb_premul(&row[x], r, g, b, final_a);
        }
    }
}

static void rasterize_rect_fallback(const gpu_svg_document_t* document,
                                    float scale, float tx, float ty,
                                    unsigned char* out_rgba, int buf_w, int buf_h, int stride)
{
    if(document == nullptr || document->source == nullptr)
        return;

    const char* p = document->source;
    while((p = std::strstr(p, "<rect")) != nullptr) {
        const char* tag_end = std::strchr(p, '>');
        if(tag_end == nullptr)
            break;

        float x = parse_attr_float(p, "x", 0.0f);
        float y = parse_attr_float(p, "y", 0.0f);
        float w = parse_attr_float(p, "width", 0.0f);
        float h = parse_attr_float(p, "height", 0.0f);
        float rx = parse_attr_float(p, "rx", 0.0f);
        float ry = parse_attr_float(p, "ry", rx);
        float stroke_w_f = parse_attr_float(p, "stroke-width", 0.0f);
        int sx = static_cast<int>(x * scale + tx);
        int sy = static_cast<int>(y * scale + ty);
        int sw = static_cast<int>(w * scale + 0.999f);
        int sh = static_cast<int>(h * scale + 0.999f);
        int srx = static_cast<int>(rx * scale + 0.5f);
        int sry = static_cast<int>(ry * scale + 0.5f);
        if(srx > sw / 2) srx = sw / 2;
        if(sry > sh / 2) sry = sh / 2;
        if(srx < 0) srx = 0;
        if(sry < 0) sry = 0;

        unsigned char r = 0, g = 0, b = 0, a = 0;
        if(w > 0.0f && h > 0.0f && parse_paint_rgba(p, "fill", "fill-opacity", r, g, b, a)) {
            fill_rounded_rect_fallback(out_rgba, buf_w, buf_h, stride, sx, sy, sw, sh, srx, sry, r, g, b, a);
        }

        if(stroke_w_f > 0.0f) {
            unsigned char sr = 0, sg = 0, sb = 0, sa = 0;
            if(parse_paint_rgba(p, "stroke", "stroke-opacity", sr, sg, sb, sa)) {
                int ssw = static_cast<int>(stroke_w_f * scale + 0.5f);
                if(ssw < 1) ssw = 1;
                stroke_rounded_rect_fallback(out_rgba, buf_w, buf_h, stride, sx, sy, sw, sh, srx, sry, ssw, sr, sg, sb, sa);
            }
        }

        p = tag_end + 1;
    }
}

static void rasterize_rect_fallback_premul(const gpu_svg_document_t* document,
                                           float scale, float tx, float ty,
                                           unsigned char* out_argb, int buf_w, int buf_h, int stride)
{
    if(document == nullptr || document->source == nullptr)
        return;

    const char* p = document->source;
    while((p = std::strstr(p, "<rect")) != nullptr) {
        const char* tag_end = std::strchr(p, '>');
        if(tag_end == nullptr)
            break;

        float x = parse_attr_float(p, "x", 0.0f);
        float y = parse_attr_float(p, "y", 0.0f);
        float w = parse_attr_float(p, "width", 0.0f);
        float h = parse_attr_float(p, "height", 0.0f);
        float rx = parse_attr_float(p, "rx", 0.0f);
        float ry = parse_attr_float(p, "ry", rx);
        float stroke_w_f = parse_attr_float(p, "stroke-width", 0.0f);
        int sx = static_cast<int>(x * scale + tx);
        int sy = static_cast<int>(y * scale + ty);
        int sw = static_cast<int>(w * scale + 0.999f);
        int sh = static_cast<int>(h * scale + 0.999f);
        int srx = static_cast<int>(rx * scale + 0.5f);
        int sry = static_cast<int>(ry * scale + 0.5f);
        if(srx > sw / 2) srx = sw / 2;
        if(sry > sh / 2) sry = sh / 2;
        if(srx < 0) srx = 0;
        if(sry < 0) sry = 0;

        unsigned char r = 0, g = 0, b = 0, a = 0;
        if(w > 0.0f && h > 0.0f && parse_paint_rgba(p, "fill", "fill-opacity", r, g, b, a)) {
            fill_rounded_rect_fallback_premul(out_argb, buf_w, buf_h, stride, sx, sy, sw, sh, srx, sry, r, g, b, a);
        }

        if(stroke_w_f > 0.0f) {
            unsigned char sr = 0, sg = 0, sb = 0, sa = 0;
            if(parse_paint_rgba(p, "stroke", "stroke-opacity", sr, sg, sb, sa)) {
                int ssw = static_cast<int>(stroke_w_f * scale + 0.5f);
                if(ssw < 1) ssw = 1;
                stroke_rounded_rect_fallback_premul(out_argb, buf_w, buf_h, stride, sx, sy, sw, sh, srx, sry, ssw, sr, sg, sb, sa);
            }
        }

        p = tag_end + 1;
    }
}

static void rasterize_squircle_markers_rgba(const gpu_svg_document_t* document,
                                            float scale, float tx, float ty,
                                            unsigned char* out_rgba, int buf_w, int buf_h, int stride)
{
    if(document == nullptr || document->source == nullptr)
        return;
    const char* p = document->source;
    const char* marker = "<!--BARAM_SQUIRCLE ";
    while((p = std::strstr(p, marker)) != nullptr) {
        const char* end = std::strstr(p, "-->");
        if(end == nullptr)
            break;

        float x = parse_attr_float(p, "x", 0.0f);
        float y = parse_attr_float(p, "y", 0.0f);
        float w = parse_attr_float(p, "w", 0.0f);
        float h = parse_attr_float(p, "h", 0.0f);
        float radius = parse_attr_float(p, "radius", -1.0f);
        float stroke_w_f = parse_attr_float(p, "stroke-width", 0.0f);

        unsigned char fr=0, fg=0, fb=0, fa=0;
        unsigned char sr=0, sg=0, sb=0, sa=0;
        bool has_fill = parse_paint_rgba(p, "fill", "fill-opacity", fr, fg, fb, fa);
        bool has_stroke = parse_paint_rgba(p, "stroke", "stroke-opacity", sr, sg, sb, sa);

        std::vector<SquirclePoint> outer;
        build_squircle_polygon(x * scale + tx, y * scale + ty, static_cast<int>(w * scale + 0.999f),
                               static_cast<int>(h * scale + 0.999f), radius, outer);
        std::vector<SquirclePoint> inner;
        int ssw = static_cast<int>(stroke_w_f * scale + 0.5f);
        if(ssw < 1) ssw = 1;
        if(has_stroke && stroke_w_f > 0.0f && w * scale - 2.0f * ssw > 0.0f && h * scale - 2.0f * ssw > 0.0f) {
            build_squircle_polygon(x * scale + tx + ssw, y * scale + ty + ssw,
                                   static_cast<int>(w * scale + 0.999f) - ssw * 2,
                                   static_cast<int>(h * scale + 0.999f) - ssw * 2,
                                   radius, inner);
        }

        int min_x = static_cast<int>(x * scale + tx);
        int min_y = static_cast<int>(y * scale + ty);
        int max_x = min_x + static_cast<int>(w * scale + 0.999f);
        int max_y = min_y + static_cast<int>(h * scale + 0.999f);
        if(min_x < 0) min_x = 0;
        if(min_y < 0) min_y = 0;
        if(max_x > buf_w) max_x = buf_w;
        if(max_y > buf_h) max_y = buf_h;
        for(int yy = min_y; yy < max_y; ++yy) {
            unsigned char* row = out_rgba + static_cast<size_t>(yy) * static_cast<size_t>(stride);
            for(int xx = min_x; xx < max_x; ++xx) {
                unsigned char outer_cov = squircle_coverage(xx, yy, outer);
                if(outer_cov == 0)
                    continue;
                unsigned char inner_cov = inner.empty() ? 0 : squircle_coverage(xx, yy, inner);
                if(has_fill) {
                    unsigned char fill_cov = outer_cov;
                    if(fill_cov) {
                        unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(fa) * fill_cov + 127u) / 255u);
                        blend_pixel(row + xx * 4, fr, fg, fb, final_a);
                    }
                }
                if(has_stroke && outer_cov > inner_cov) {
                    unsigned char edge_cov = static_cast<unsigned char>(outer_cov - inner_cov);
                    unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(sa) * edge_cov + 127u) / 255u);
                    blend_pixel(row + xx * 4, sr, sg, sb, final_a);
                }
            }
        }
        p = end + 3;
    }
}

static void rasterize_squircle_markers_premul(const gpu_svg_document_t* document,
                                              float scale, float tx, float ty,
                                              unsigned char* out_argb, int buf_w, int buf_h, int stride)
{
    if(document == nullptr || document->source == nullptr)
        return;
    const char* p = document->source;
    const char* marker = "<!--BARAM_SQUIRCLE ";
    while((p = std::strstr(p, marker)) != nullptr) {
        const char* end = std::strstr(p, "-->");
        if(end == nullptr)
            break;

        float x = parse_attr_float(p, "x", 0.0f);
        float y = parse_attr_float(p, "y", 0.0f);
        float w = parse_attr_float(p, "w", 0.0f);
        float h = parse_attr_float(p, "h", 0.0f);
        float radius = parse_attr_float(p, "radius", -1.0f);
        float stroke_w_f = parse_attr_float(p, "stroke-width", 0.0f);
        unsigned char fr=0, fg=0, fb=0, fa=0;
        unsigned char sr=0, sg=0, sb=0, sa=0;
        bool has_fill = parse_paint_rgba(p, "fill", "fill-opacity", fr, fg, fb, fa);
        bool has_stroke = parse_paint_rgba(p, "stroke", "stroke-opacity", sr, sg, sb, sa);

        std::vector<SquirclePoint> outer;
        build_squircle_polygon(x * scale + tx, y * scale + ty, static_cast<int>(w * scale + 0.999f),
                               static_cast<int>(h * scale + 0.999f), radius, outer);
        std::vector<SquirclePoint> inner;
        int ssw = static_cast<int>(stroke_w_f * scale + 0.5f);
        if(ssw < 1) ssw = 1;
        if(has_stroke && stroke_w_f > 0.0f && w * scale - 2.0f * ssw > 0.0f && h * scale - 2.0f * ssw > 0.0f) {
            build_squircle_polygon(x * scale + tx + ssw, y * scale + ty + ssw,
                                   static_cast<int>(w * scale + 0.999f) - ssw * 2,
                                   static_cast<int>(h * scale + 0.999f) - ssw * 2,
                                   radius, inner);
        }

        int min_x = static_cast<int>(x * scale + tx);
        int min_y = static_cast<int>(y * scale + ty);
        int max_x = min_x + static_cast<int>(w * scale + 0.999f);
        int max_y = min_y + static_cast<int>(h * scale + 0.999f);
        if(min_x < 0) min_x = 0;
        if(min_y < 0) min_y = 0;
        if(max_x > buf_w) max_x = buf_w;
        if(max_y > buf_h) max_y = buf_h;
        for(int yy = min_y; yy < max_y; ++yy) {
            uint32_t* row = reinterpret_cast<uint32_t*>(out_argb + static_cast<size_t>(yy) * static_cast<size_t>(stride));
            for(int xx = min_x; xx < max_x; ++xx) {
                unsigned char outer_cov = squircle_coverage(xx, yy, outer);
                if(outer_cov == 0)
                    continue;
                unsigned char inner_cov = inner.empty() ? 0 : squircle_coverage(xx, yy, inner);
                if(has_fill) {
                    unsigned char fill_cov = outer_cov;
                    if(fill_cov) {
                        unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(fa) * fill_cov + 127u) / 255u);
                        blend_pixel_argb_premul(&row[xx], fr, fg, fb, final_a);
                    }
                }
                if(has_stroke && outer_cov > inner_cov) {
                    unsigned char edge_cov = static_cast<unsigned char>(outer_cov - inner_cov);
                    unsigned char final_a = static_cast<unsigned char>((static_cast<unsigned>(sa) * edge_cov + 127u) / 255u);
                    blend_pixel_argb_premul(&row[xx], sr, sg, sb, final_a);
                }
            }
        }
        p = end + 3;
    }
}

static bool buffer_has_alpha(const unsigned char* out_rgba, int buf_w, int buf_h, int stride)
{
    if(out_rgba == nullptr)
        return false;
    for(int y = 0; y < buf_h; ++y) {
        const unsigned char* row = out_rgba + static_cast<size_t>(y) * static_cast<size_t>(stride);
        for(int x = 0; x < buf_w; ++x) {
            if(row[x * 4 + 3] != 0)
                return true;
        }
    }
    return false;
}

int gpu_svg_init(gpu_svg_renderer_t* renderer, int width, int height)
{
    if(renderer == nullptr)
        return -1;

    renderer->width = (uint32_t)width;
    renderer->height = (uint32_t)height;
    renderer->scale = 1.0f;
    renderer->tx = 0.0f;
    renderer->ty = 0.0f;
    renderer->vertex_cap = 0;
    renderer->vertex_count = 0;
    renderer->vertices = nullptr;
    return 0;
}

gpu_svg_document_t* gpu_svg_parse(const char* svg_data)
{
    if(svg_data == nullptr)
        return nullptr;

    auto document = lunasvg::Document::loadFromData(svg_data, std::strlen(svg_data));
    if(!document)
        return nullptr;

    gpu_svg_document_t* result = new gpu_svg_document_t;
    result->width = document->width();
    result->height = document->height();
    result->document = std::move(document);
    size_t svg_len = std::strlen(svg_data);
    result->source = static_cast<char*>(std::malloc(svg_len + 1));
    if(result->source != nullptr)
        std::memcpy(result->source, svg_data, svg_len + 1);
    return result;
}

void gpu_svg_delete(gpu_svg_document_t* document)
{
    if(document && document->source)
        std::free(document->source);
    delete document;
}

float gpu_svg_width(const gpu_svg_document_t* document)
{
    return document ? document->width : 0.0f;
}

float gpu_svg_height(const gpu_svg_document_t* document)
{
    return document ? document->height : 0.0f;
}

int gpu_svg_rasterize(const gpu_svg_document_t* document,
                      float scale, float tx, float ty,
                      unsigned char* out_rgba,
                      int buf_w, int buf_h, int stride)
{
    if(document == nullptr || document->document == nullptr || out_rgba == nullptr)
        return -1;
    if(buf_w <= 0 || buf_h <= 0 || stride < buf_w * 4)
        return -1;

    std::memset(out_rgba, 0, (size_t)stride * (size_t)buf_h);
    lunasvg::Bitmap bitmap(out_rgba, buf_w, buf_h, stride);
    bitmap.clear(0x00000000u);
    document->document->render(bitmap, lunasvg::Matrix(scale, 0.0f, 0.0f, scale, tx, ty));
    bitmap.convertToRGBA();
    if(document->source != nullptr && std::strstr(document->source, "<!--BARAM_SQUIRCLE ") != nullptr) {
        rasterize_squircle_markers_rgba(document, scale, tx, ty, out_rgba, buf_w, buf_h, stride);
    }
    if(!buffer_has_alpha(out_rgba, buf_w, buf_h, stride) && document->source != nullptr) {
        if(std::strstr(document->source, "<rect") != nullptr)
            rasterize_rect_fallback(document, scale, tx, ty, out_rgba, buf_w, buf_h, stride);
    }
    return 0;
}

int gpu_svg_rasterize_premul(const gpu_svg_document_t* document,
                             float scale, float tx, float ty,
                             unsigned char* out_argb_premul,
                             int buf_w, int buf_h, int stride)
{
    if(document == nullptr || document->document == nullptr || out_argb_premul == nullptr)
        return -1;
    if(buf_w <= 0 || buf_h <= 0 || stride < buf_w * 4)
        return -1;

    std::memset(out_argb_premul, 0, static_cast<size_t>(stride) * static_cast<size_t>(buf_h));
    lunasvg::Bitmap bitmap(out_argb_premul, buf_w, buf_h, stride);
    bitmap.clear(0x00000000u);
    document->document->render(bitmap, lunasvg::Matrix(scale, 0.0f, 0.0f, scale, tx, ty));
    if(document->source != nullptr && std::strstr(document->source, "<!--BARAM_SQUIRCLE ") != nullptr) {
        rasterize_squircle_markers_premul(document, scale, tx, ty, out_argb_premul, buf_w, buf_h, stride);
    }
    if(!buffer_has_alpha(out_argb_premul, buf_w, buf_h, stride) && document->source != nullptr) {
        if(std::strstr(document->source, "<rect") != nullptr)
            rasterize_rect_fallback_premul(document, scale, tx, ty, out_argb_premul, buf_w, buf_h, stride);
    }
    return 0;
}

int gpu_svg_render(gpu_svg_renderer_t* renderer, const gpu_svg_document_t* document,
                   float scale, float tx, float ty,
                   uint32_t* out_buffer, int buf_w, int buf_h)
{
    (void)renderer;
    return gpu_svg_rasterize(document, scale, tx, ty, (unsigned char*)out_buffer,
                             buf_w, buf_h, buf_w * 4);
}

void gpu_svg_cleanup(gpu_svg_renderer_t* renderer)
{
    if(renderer && renderer->vertices) {
        delete[] renderer->vertices;
        renderer->vertices = nullptr;
    }
}
