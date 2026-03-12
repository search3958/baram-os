#ifndef SVG_DATA_H
#define SVG_DATA_H

#include <stddef.h>

#define NOTE_TEST_SVG_WIDTH 1280
#define NOTE_TEST_SVG_HEIGHT 720

// CLASSICモード用のテストSVG
// ID="conic" を付与することで、カーネル側で円錐状グラデーションを適用します
static const unsigned char note_test_svg[] = 
    "<svg width=\"1280\" height=\"720\" viewBox=\"0 0 1280 720\" fill=\"none\" xmlns=\"http://www.w3.org/2000/svg\">"
    "<rect width=\"1280\" height=\"720\" fill=\"#1a1a1a\"/>"
    
    "<!-- 中央の円錐グラデーション円 -->"
    "<circle id=\"conic\" cx=\"640\" cy=\"360\" r=\"200\" fill=\"white\" />"
    
    "<!-- 装飾用の小さな円 -->"
    "<circle id=\"conic_red\" cx=\"200\" cy=\"200\" r=\"80\" fill=\"white\" />"
    "<circle id=\"conic_black\" cx=\"1080\" cy=\"520\" r=\"120\" fill=\"white\" />"
    
    "<text x=\"640\" y=\"650\" fill=\"white\" font-family=\"sans-serif\" font-size=\"24\" text-anchor=\"middle\">CLASSIC MODE - CONIC GRADIENT TEST</text>"
    "</svg>";

static const size_t note_test_svg_len = sizeof(note_test_svg) - 1;

#endif
