#include "gpu_svg.h"

#include "../lunasvg/include/lunasvg.h"

#include <cctype>
#include <cstring>
#include <cstdlib>
#include <memory>
struct gpu_svg_document {
    std::unique_ptr<lunasvg::Document> document;
    float width;
    float height;
    char* source;
};

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

    size_t attr_len = std::strlen(attr);
    const char* p = tag;
    while((p = std::strstr(p, attr)) != nullptr) {
        if(p != tag) {
            char prev = p[-1];
            if(std::isalnum(static_cast<unsigned char>(prev)) || prev == '_' || prev == '-') {
                p += attr_len;
                continue;
            }
        }
        const char* q = skip_spaces(p + attr_len);
        if(q == nullptr || *q != '=') {
            p += attr_len;
            continue;
        }
        q = skip_spaces(q + 1);
        if(q == nullptr || (*q != '"' && *q != '\''))
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

    if(len == 7) {
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
    a = static_cast<unsigned char>(opacity * 255.0f + 0.5f);
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
    px[0] = static_cast<unsigned char>(sr + ((px[0] * inv_sa + 127u) / 255u));
    px[1] = static_cast<unsigned char>(sg + ((px[1] * inv_sa + 127u) / 255u));
    px[2] = static_cast<unsigned char>(sb + ((px[2] * inv_sa + 127u) / 255u));
    px[3] = static_cast<unsigned char>(out_a > 255u ? 255u : out_a);
}

static bool point_in_rounded_rect(int x, int y, int w, int h, int rx, int ry)
{
    if(w <= 0 || h <= 0)
        return false;
    if(rx <= 0 || ry <= 0)
        return x >= 0 && y >= 0 && x < w && y < h;

    if(x < 0 || y < 0 || x >= w || y >= h)
        return false;

    if((x >= rx && x < w - rx) || (y >= ry && y < h - ry))
        return true;

    int cx = (x < rx) ? rx : (w - rx - 1);
    int cy = (y < ry) ? ry : (h - ry - 1);
    long long dx = static_cast<long long>(x - cx);
    long long dy = static_cast<long long>(y - cy);
    long long lhs = dx * dx * static_cast<long long>(ry) * static_cast<long long>(ry) +
                    dy * dy * static_cast<long long>(rx) * static_cast<long long>(rx);
    long long rhs = static_cast<long long>(rx) * static_cast<long long>(rx) *
                    static_cast<long long>(ry) * static_cast<long long>(ry);
    return lhs <= rhs;
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
            if(!point_in_rounded_rect(x - orig_x0, y - orig_y0, w, h, rx, ry))
                continue;
            blend_pixel(row + x * 4, r, g, b, a);
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
            if(!point_in_rounded_rect(lx, ly, w, h, rx, ry))
                continue;
            if(inner_w > 0 && inner_h > 0 &&
               point_in_rounded_rect(lx - stroke_w, ly - stroke_w, inner_w, inner_h, inner_rx, inner_ry))
                continue;
            blend_pixel(row + x * 4, r, g, b, a);
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
    if(!buffer_has_alpha(out_rgba, buf_w, buf_h, stride) && document->source != nullptr &&
       std::strstr(document->source, "<rect") != nullptr) {
        rasterize_rect_fallback(document, scale, tx, ty, out_rgba, buf_w, buf_h, stride);
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
