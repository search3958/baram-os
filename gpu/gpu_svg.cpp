#include "gpu_svg.h"

#include "../lunasvg/include/lunasvg.h"

#include <cstring>
#include <memory>
struct gpu_svg_document {
    std::unique_ptr<lunasvg::Document> document;
    float width;
    float height;
};

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
    return result;
}

void gpu_svg_delete(gpu_svg_document_t* document)
{
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
