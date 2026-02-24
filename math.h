#ifndef _MATH_H_
#define _MATH_H_

// Minimal math.h for freestanding kernel + NanoSVG
// Note: x87 FPU inline ASM ("=t" constraint) is GCC-specific and not supported
// by Clang. All functions are implemented in pure C for portability.

// ---- float / double 演算 (純粋C実装) ----

// --- sqrtf / sqrt: ニュートン法 ---
static inline float sqrtf(float x) {
  if (x < 0.0f)
    return 0.0f;
  if (x == 0.0f)
    return 0.0f;
  float r = x > 1.0f ? x : 1.0f;
  // 20回のニュートン法で十分な精度
  for (int i = 0; i < 20; i++)
    r = 0.5f * (r + x / r);
  return r;
}

static inline double sqrt(double x) {
  if (x < 0.0)
    return 0.0;
  if (x == 0.0)
    return 0.0;
  double r = x > 1.0 ? x : 1.0;
  for (int i = 0; i < 30; i++)
    r = 0.5 * (r + x / r);
  return r;
}

// --- fabs / fabsf ---
static inline float fabsf(float x) { return x < 0.0f ? -x : x; }
static inline double fabs(double x) { return x < 0.0 ? -x : x; }

// --- floor / floorf ---
static inline float floorf(float x) {
  int i = (int)x;
  return (float)(x < (float)i ? i - 1 : i);
}
static inline double floor(double x) {
  long long i = (long long)x;
  return (double)(x < (double)i ? i - 1 : i);
}

// --- ceil / ceilf ---
static inline float ceilf(float x) {
  int i = (int)x;
  return (float)(x > (float)i ? i + 1 : i);
}
static inline double ceil(double x) {
  long long i = (long long)x;
  return (double)(x > (double)i ? i + 1 : i);
}

// --- round / roundf ---
static inline float roundf(float x) {
  return (x >= 0.0f) ? floorf(x + 0.5f) : ceilf(x - 0.5f);
}
static inline double round(double x) {
  return (x >= 0.0) ? floor(x + 0.5) : ceil(x - 0.5);
}

// --- cosf: マクローリン展開 (20項) ---
static inline float cosf(float x) {
  // x を [-π, π] に正規化
  // π の近似
  const float PI = 3.14159265358979323846f;
  const float TWO_PI = 6.28318530717958647692f;
  // 範囲を [-2π, 2π] 付近に収める
  while (x > PI)
    x -= TWO_PI;
  while (x < -PI)
    x += TWO_PI;
  float result = 1.0f;
  float term = 1.0f;
  float x2 = x * x;
  for (int n = 1; n <= 10; n++) {
    term *= -x2 / (float)((2 * n - 1) * (2 * n));
    result += term;
  }
  return result;
}

// --- sinf: マクローリン展開 (20項) ---
static inline float sinf(float x) {
  const float PI = 3.14159265358979323846f;
  const float TWO_PI = 6.28318530717958647692f;
  while (x > PI)
    x -= TWO_PI;
  while (x < -PI)
    x += TWO_PI;
  float result = x;
  float term = x;
  float x2 = x * x;
  for (int n = 1; n <= 10; n++) {
    term *= -x2 / (float)((2 * n) * (2 * n + 1));
    result += term;
  }
  return result;
}

// --- tanf ---
static inline float tanf(float x) {
  float c = cosf(x);
  if (c == 0.0f)
    return 0.0f; // ゼロ除算回避
  return sinf(x) / c;
}

// --- atan2f: CORDIC近似 ---
static inline float atan2f(float y, float x) {
  const float PI = 3.14159265358979323846f;
  if (x == 0.0f) {
    if (y > 0.0f)
      return PI / 2.0f;
    if (y < 0.0f)
      return -PI / 2.0f;
    return 0.0f;
  }
  float t = y / x;
  // arctan(t) のマクローリン展開（収束を速めるため |t|<=1 に変換）
  float atan_val;
  float at = t < 0.0f ? -t : t; // fabsf(t)
  if (at <= 1.0f) {
    float t2 = t * t;
    atan_val = t;
    float term = t;
    for (int n = 1; n <= 10; n++) {
      term *= -t2;
      atan_val += term / (float)(2 * n + 1);
    }
  } else {
    // |t| > 1: atan(t) = ±π/2 - atan(1/t)
    float inv = 1.0f / t;
    float t2 = inv * inv;
    float atan_inv = inv;
    float term = inv;
    for (int n = 1; n <= 10; n++) {
      term *= -t2;
      atan_inv += term / (float)(2 * n + 1);
    }
    atan_val = (t > 0.0f ? PI / 2.0f : -PI / 2.0f) - atan_inv;
  }
  // 象限補正
  if (x < 0.0f) {
    atan_val += (y >= 0.0f) ? PI : -PI;
  }
  return atan_val;
}

// --- acosf / asinf ---
static inline float acosf(float x) { return atan2f(sqrtf(1.0f - x * x), x); }
static inline float asinf(float x) { return atan2f(x, sqrtf(1.0f - x * x)); }

// --- fmodf / fmod ---
static inline float fmodf(float x, float y) {
  if (y == 0.0f)
    return 0.0f;
  int q = (int)(x / y);
  return x - (float)q * y;
}
static inline double fmod(double x, double y) {
  if (y == 0.0)
    return 0.0;
  long long q = (long long)(x / y);
  return x - (double)q * y;
}

// --- pow: 整数乗のみ（nanosvg の pow(10.0, n) 用途） ---
static inline double pow(double base, double exp) {
  if (exp == 0.0)
    return 1.0;
  double result = 1.0;
  int negative = 0;
  if (exp < 0.0) {
    negative = 1;
    exp = -exp;
  }
  long long n = (long long)exp;
  for (long long i = 0; i < n; i++)
    result *= base;
  return negative ? 1.0 / result : result;
}

static inline float powf(float base, float exp) {
  return (float)pow((double)base, (double)exp);
}

// --- log10f: ダミー（nanosvg内では実用されない） ---
static inline float log10f(float x) {
  (void)x;
  return 0.0f;
}

// --- isnan: IEEE754 では NaN は自分自身と等しくない ---
static inline int isnan(double x) { return x != x; }
static inline int isnanf(float x) { return x != x; }

#endif
