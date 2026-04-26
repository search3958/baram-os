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

static bool parse_fill_rgba(const char* tag, unsigned char& r, unsigned char& g,
                            unsigned char& b, unsigned char& a)
{
    const char* fill = find_attr_value(tag, "fill");
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
    if(opacity < 0.0f) opacity = 0.0f;
    if(opacity > 1.0f) opacity = 1.0f;
    a = static_cast<unsigned char>(opacity * 255.0f + 0.5f);
    return a != 0;
}

static void fill_rect_fallback(unsigned char* out_rgba, int buf_w, int buf_h, int stride,
                               int x0, int y0, int x1, int y1,
                               unsigned char r, unsigned char g, unsigned char b, unsigned char a)
{
    if(x0 < 0) x0 = 0;
    if(y0 < 0) y0 = 0;
    if(x1 > buf_w) x1 = buf_w;
    if(y1 > buf_h) y1 = buf_h;
    if(x0 >= x1 || y0 >= y1 || a == 0)
        return;

    for(int y = y0; y < y1; ++y) {
        unsigned char* row = out_rgba + static_cast<size_t>(y) * static_cast<size_t>(stride);
        for(int x = x0; x < x1; ++x) {
            unsigned char* px = row + x * 4;
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = a;
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
        unsigned char r = 0, g = 0, b = 0, a = 0;
        if(w > 0.0f && h > 0.0f && parse_fill_rgba(p, r, g, b, a)) {
            int x0 = static_cast<int>(x * scale + tx);
            int y0 = static_cast<int>(y * scale + ty);
            int x1 = static_cast<int>((x + w) * scale + tx + 0.999f);
            int y1 = static_cast<int>((y + h) * scale + ty + 0.999f);
            fill_rect_fallback(out_rgba, buf_w, buf_h, stride, x0, y0, x1, y1, r, g, b, a);
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
