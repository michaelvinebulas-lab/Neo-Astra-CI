//! Embedded C runtime for the Neo Astra C backend.
//!
//! The generated translation unit is freestanding-safe: it includes only
//! `<stdint.h>`, `<stdbool.h>`, and `<stddef.h>` and declares the runtime as
//! `extern`. The [`HOST_SHIM`] is a separate `astra_host.c` that defines those
//! externs using libc; it is linked only when a hosted executable is needed.
//! This keeps the generated code honest about its runtime boundary (design
//! §6.3 "the runtime is linked only for the features used") instead of hiding
//! a private runtime behind `#include`.

/// Fixed-width value type declarations and `extern` runtime prototypes emitted
/// at the top of every generated `.c`. Uses only freestanding clang builtins.
pub const RUNTIME_HEADER: &str = r#"#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/* Neo Astra C runtime -- freestanding declarations. */
#define ASTRA_NORETURN __attribute__((noreturn))

/* Managed value types shared by all generated modules. */
typedef struct { const char* data; size_t len; } NS_Str;
typedef struct { const uint8_t* data; size_t len; } NS_Bytes;
typedef struct { void* data; size_t len; size_t cap; } NS_Array;
typedef struct { void* keys; void* values; size_t len; size_t cap; size_t key_size; size_t val_size; bool key_is_str; } NS_Map;
typedef struct { char _unit; } NS_Unit;
/* Built-in nominal error payload for `str.slice` (spec §6.2). Declared here
   rather than per module so every translation unit shares one stem-free type. */
typedef struct { NS_Str _0; } NS_SliceError;
/* Built-in fallible `str.to_int` and integer overflow payloads. The `_0`
   field carries the offending text, matching the tuple-struct shapes the
   checker and the other backends use. */
typedef struct { NS_Str _0; } NS_ParseIntError;
typedef struct { NS_Str _0; } NS_OverflowError;

/* Console builtins. Each `print`/`println` is typed by its argument type. */
extern void astra_print_i8(int8_t x);
extern void astra_print_i16(int16_t x);
extern void astra_print_i32(int32_t x);
extern void astra_print_i64(int64_t x);
extern void astra_print_u8(uint8_t x);
extern void astra_print_u16(uint16_t x);
extern void astra_print_u32(uint32_t x);
extern void astra_print_u64(uint64_t x);
extern void astra_print_usize(size_t x);
extern void astra_print_isize(intptr_t x);
extern void astra_print_f32(float x);
extern void astra_print_f64(double x);
extern void astra_print_bool(bool x);
extern void astra_print_str(NS_Str x);
extern void astra_print_bytes(NS_Bytes x);
extern void astra_print_unit(void);

extern void astra_println_i8(int8_t x);
extern void astra_println_i16(int16_t x);
extern void astra_println_i32(int32_t x);
extern void astra_println_i64(int64_t x);
extern void astra_println_u8(uint8_t x);
extern void astra_println_u16(uint16_t x);
extern void astra_println_u32(uint32_t x);
extern void astra_println_u64(uint64_t x);
extern void astra_println_usize(size_t x);
extern void astra_println_isize(intptr_t x);
extern void astra_println_f32(float x);
extern void astra_println_f64(double x);
extern void astra_println_bool(bool x);
extern void astra_println_str(NS_Str x);
extern void astra_println_bytes(NS_Bytes x);
extern void astra_println_unit(void);

/* Failure and control builtins. */
extern void astra_assert(bool cond);
ASTRA_NORETURN extern void astra_panic(const char* msg);
ASTRA_NORETURN extern void astra_abort(const char* msg);

/* Managed allocation and containers. */
extern void* astra_alloc(size_t n);
extern NS_Array astra_array_new_lit(size_t n, size_t elem_size, const void* src);
extern void astra_array_push(NS_Array* arr, size_t elem_size, const void* value);
extern void* astra_array_index(NS_Array* arr, size_t i, size_t elem_size);

/* String helpers. */
extern size_t astra_str_len(NS_Str s);
extern bool astra_str_is_empty(NS_Str s);
extern bool astra_str_eq(NS_Str a, NS_Str b);
extern int astra_str_cmp(NS_Str a, NS_Str b);
/* Fallible byte-offset slice for `str.slice(start, end)` (spec §6.2): returns
   false when the range is reversed, out of bounds, or splits a UTF-8 scalar. */
extern bool astra_str_slice_try(NS_Str s, size_t start, size_t end, NS_Str* out);
/* Strict decimal parse for `str.to_int()` (spec §6.2): false unless the whole
   string is an optional sign plus decimal digits that fit `int32_t`. */
extern bool astra_str_to_int_try(NS_Str s, int32_t* out);

/* Bytes helpers. */
extern bool astra_bytes_eq(NS_Bytes a, NS_Bytes b);
extern int astra_bytes_cmp(NS_Bytes a, NS_Bytes b);
extern uint8_t astra_bytes_get(NS_Bytes b, size_t i);
/* Infallible byte-offset slice for `bytes.slice(start, end)`; bounds-checked,
   so a reversed or out-of-range panics. */
extern NS_Bytes astra_bytes_slice(NS_Bytes b, size_t start, size_t end);

/* Map helpers. */
extern NS_Map astra_map_new(size_t key_size, size_t val_size, bool key_is_str);
extern void astra_map_set(NS_Map* m, const void* key, const void* val);
extern void* astra_map_get(const NS_Map* m, const void* key);
extern bool astra_map_contains(const NS_Map* m, const void* key);
extern size_t astra_map_len(const NS_Map* m);
extern void* astra_map_key_at(const NS_Map* m, size_t i);

/* Integer overflow helpers (signed wrapping via unsigned round-trip). */
extern int8_t  astra_add_i8(int8_t a, int8_t b);
extern int16_t astra_add_i16(int16_t a, int16_t b);
extern int32_t astra_add_i32(int32_t a, int32_t b);
extern int64_t astra_add_i64(int64_t a, int64_t b);
extern int8_t  astra_sub_i8(int8_t a, int8_t b);
extern int16_t astra_sub_i16(int16_t a, int16_t b);
extern int32_t astra_sub_i32(int32_t a, int32_t b);
extern int64_t astra_sub_i64(int64_t a, int64_t b);
extern int8_t  astra_mul_i8(int8_t a, int8_t b);
extern int16_t astra_mul_i16(int16_t a, int16_t b);
extern int32_t astra_mul_i32(int32_t a, int32_t b);
extern int64_t astra_mul_i64(int64_t a, int64_t b);
"#;

/// The hosted runtime implementation, written to `astra_host.c` and linked only
/// when a runnable executable is produced. It implements the `extern` shims
/// declared in [`RUNTIME_HEADER`] using the C standard library.
pub const HOST_SHIM: &str = r#"/* Neo Astra C runtime -- hosted implementation (astra_host.c). */
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { const char* data; size_t len; } NS_Str;
typedef struct { void* data; size_t len; size_t cap; } NS_Array;

static void astra_write_str(NS_Str x) {
    if (x.len > 0) {
        fwrite(x.data, 1, x.len, stdout);
    }
}

/* Float rendering matches the Nova interpreter value renderer: integral values
   print with one decimal place, non-integral values use the shortest form. */
static void astra_write_f64(double x) {
    if (x != x) { fputs("NaN", stdout); return; }
    if (x == (double)INFINITY) { fputs("Infinity", stdout); return; }
    if (x == (double)-INFINITY) { fputs("-Infinity", stdout); return; }
    double f = x - (double)(int64_t)x;
    if (f == 0.0 && x < 1e16 && x > -1e16) {
        printf("%.1f", x);
    } else {
        printf("%g", x);
    }
}

static void astra_write_f32(float x) { astra_write_f64((double)x); }

#define ASTRA_PRINT_INT(name, type, signed_fmt, unsigned_fmt) \
    void name(type x) { printf(x < 0 ? signed_fmt : unsigned_fmt, x); }

void astra_print_i8(int8_t x) { printf(x < 0 ? "%" PRId8 : "%" PRIu8, x); }
void astra_print_i16(int16_t x) { printf(x < 0 ? "%" PRId16 : "%" PRIu16, x); }
void astra_print_i32(int32_t x) { printf(x < 0 ? "%" PRId32 : "%" PRIu32, x); }
void astra_print_i64(int64_t x) { printf(x < 0 ? "%" PRId64 : "%" PRIu64, x); }
void astra_print_u8(uint8_t x) { printf("%" PRIu8, x); }
void astra_print_u16(uint16_t x) { printf("%" PRIu16, x); }
void astra_print_u32(uint32_t x) { printf("%" PRIu32, x); }
void astra_print_u64(uint64_t x) { printf("%" PRIu64, x); }
void astra_print_usize(size_t x) { printf("%zu", x); }
void astra_print_isize(intptr_t x) { printf("%td", x); }
void astra_print_f32(float x) { astra_write_f32(x); }
void astra_print_f64(double x) { astra_write_f64(x); }
void astra_print_bool(bool x) { fputs(x ? "true" : "false", stdout); }
void astra_print_str(NS_Str x) { astra_write_str(x); }
void astra_print_bytes(NS_Bytes x) {
    for (size_t i = 0; i < x.len; i++) { printf("%02x", x.data[i]); }
}
void astra_print_unit(void) { fputs("()", stdout); }

void astra_println_i8(int8_t x) { astra_print_i8(x); putchar('\n'); }
void astra_println_i16(int16_t x) { astra_print_i16(x); putchar('\n'); }
void astra_println_i32(int32_t x) { astra_print_i32(x); putchar('\n'); }
void astra_println_i64(int64_t x) { astra_print_i64(x); putchar('\n'); }
void astra_println_u8(uint8_t x) { astra_print_u8(x); putchar('\n'); }
void astra_println_u16(uint16_t x) { astra_print_u16(x); putchar('\n'); }
void astra_println_u32(uint32_t x) { astra_print_u32(x); putchar('\n'); }
void astra_println_u64(uint64_t x) { astra_print_u64(x); putchar('\n'); }
void astra_println_usize(size_t x) { astra_print_usize(x); putchar('\n'); }
void astra_println_isize(intptr_t x) { astra_print_isize(x); putchar('\n'); }
void astra_println_f32(float x) { astra_print_f32(x); putchar('\n'); }
void astra_println_f64(double x) { astra_print_f64(x); putchar('\n'); }
void astra_println_bool(bool x) { astra_print_bool(x); putchar('\n'); }
void astra_println_str(NS_Str x) { astra_print_str(x); putchar('\n'); }
void astra_println_bytes(NS_Bytes x) { astra_print_bytes(x); putchar('\n'); }
void astra_println_unit(void) { astra_print_unit(); putchar('\n'); }

void astra_assert(bool cond) { if (!cond) { astra_panic("assertion failed"); } }
void astra_panic(const char* msg) { fprintf(stderr, "panic: %s\n", msg); abort(); }
void astra_abort(const char* msg) { fprintf(stderr, "abort: %s\n", msg); abort(); }

void* astra_alloc(size_t n) { void* p = malloc(n); if (!p) { astra_panic("out of memory"); } return p; }

NS_Array astra_array_new_lit(size_t n, size_t elem_size, const void* src) {
    NS_Array arr;
    arr.cap = n == 0 ? 1 : n;
    arr.len = n;
    arr.data = astra_alloc(arr.cap * (elem_size == 0 ? 1 : elem_size));
    if (n > 0) { memcpy(arr.data, src, n * elem_size); }
    return arr;
}

void astra_array_push(NS_Array* arr, size_t elem_size, const void* value) {
    if (arr->len >= arr->cap) {
        size_t new_cap = arr->cap == 0 ? 4 : arr->cap * 2;
        void* new_data = astra_alloc(new_cap * (elem_size == 0 ? 1 : elem_size));
        if (arr->len > 0) { memcpy(new_data, arr->data, arr->len * elem_size); }
        free(arr->data);
        arr->data = new_data;
        arr->cap = new_cap;
    }
    memcpy((char*)arr->data + arr->len * elem_size, value, elem_size);
    arr->len += 1;
}

void* astra_array_index(NS_Array* arr, size_t i, size_t elem_size) {
    if (i >= arr->len) { astra_panic("array index out of bounds"); }
    return (char*)arr->data + i * elem_size;
}

size_t astra_str_len(NS_Str s) { return s.len; }
bool astra_str_is_empty(NS_Str s) { return s.len == 0; }
bool astra_str_eq(NS_Str a, NS_Str b) {
    if (a.len != b.len) return false;
    if (a.len == 0) return true;
    return memcmp(a.data, b.data, a.len) == 0;
}

int astra_str_cmp(NS_Str a, NS_Str b) {
    size_t min_len = a.len < b.len ? a.len : b.len;
    if (min_len > 0) {
        int r = memcmp(a.data, b.data, min_len);
        if (r != 0) return r;
    }
    return a.len < b.len ? -1 : (a.len > b.len ? 1 : 0);
}

static bool astra_utf8_boundary(const char* data, size_t pos, size_t len) {
    if (pos == len) return true;
    if (pos > len) return false;
    return ((unsigned char)data[pos] & 0xC0) != 0x80;
}

bool astra_str_slice_try(NS_Str s, size_t start, size_t end, NS_Str* out) {
    if (start > end || end > s.len) return false;
    if (!astra_utf8_boundary(s.data, start, s.len)) return false;
    if (!astra_utf8_boundary(s.data, end, s.len)) return false;
    out->data = s.data + start;
    out->len = end - start;
    return true;
}

bool astra_str_to_int_try(NS_Str s, int32_t* out) {
    size_t i = 0;
    bool negative = false;
    if (i < s.len && (s.data[i] == '+' || s.data[i] == '-')) {
        negative = s.data[i] == '-';
        i += 1;
    }
    if (i >= s.len) return false;
    int64_t value = 0;
    for (; i < s.len; i += 1) {
        unsigned char digit = (unsigned char)s.data[i];
        if (digit < '0' || digit > '9') return false;
        value = value * 10 + (int64_t)(digit - '0');
        /* One past INT32_MAX leaves headroom for INT32_MIN after negation and
           keeps the accumulator far from int64 overflow. */
        if (value > 2147483648LL) return false;
    }
    if (negative) value = -value;
    if (value < -2147483648LL || value > 2147483647LL) return false;
    *out = (int32_t)value;
    return true;
}

bool astra_bytes_eq(NS_Bytes a, NS_Bytes b) {
    if (a.len != b.len) return false;
    if (a.len == 0) return true;
    return memcmp(a.data, b.data, a.len) == 0;
}

int astra_bytes_cmp(NS_Bytes a, NS_Bytes b) {
    size_t min_len = a.len < b.len ? a.len : b.len;
    if (min_len > 0) {
        int r = memcmp(a.data, b.data, min_len);
        if (r != 0) return r;
    }
    return a.len < b.len ? -1 : (a.len > b.len ? 1 : 0);
}

uint8_t astra_bytes_get(NS_Bytes b, size_t i) {
    if (i >= b.len) astra_panic("bytes index out of bounds");
    return b.data[i];
}

NS_Bytes astra_bytes_slice(NS_Bytes b, size_t start, size_t end) {
    if (start > end || end > b.len) astra_panic("bytes slice is out of bounds");
    NS_Bytes res;
    res.data = b.data + start;
    res.len = end - start;
    return res;
}

static bool astra_keys_equal(const void* a, const void* b, size_t size, bool is_str) {
    if (is_str) {
        const NS_Str* sa = (const NS_Str*)a;
        const NS_Str* sb = (const NS_Str*)b;
        if (sa->len != sb->len) return false;
        return memcmp(sa->data, sb->data, sa->len) == 0;
    }
    return memcmp(a, b, size) == 0;
}

NS_Map astra_map_new(size_t key_size, size_t val_size, bool key_is_str) {
    NS_Map m;
    m.keys = NULL;
    m.values = NULL;
    m.len = 0;
    m.cap = 0;
    m.key_size = key_size;
    m.val_size = val_size;
    m.key_is_str = key_is_str;
    return m;
}

void astra_map_set(NS_Map* m, const void* key, const void* val) {
    if (!m) return;
    char* k_bytes = (char*)m->keys;
    char* v_bytes = (char*)m->values;
    for (size_t i = 0; i < m->len; i++) {
        if (astra_keys_equal(k_bytes + i * m->key_size, key, m->key_size, m->key_is_str)) {
            memcpy(v_bytes + i * m->val_size, val, m->val_size);
            return;
        }
    }
    if (m->len >= m->cap) {
        size_t new_cap = m->cap == 0 ? 8 : m->cap * 2;
        m->keys = realloc(m->keys, new_cap * (m->key_size == 0 ? 1 : m->key_size));
        m->values = realloc(m->values, new_cap * (m->val_size == 0 ? 1 : m->val_size));
        m->cap = new_cap;
    }
    memcpy((char*)m->keys + m->len * m->key_size, key, m->key_size);
    memcpy((char*)m->values + m->len * m->val_size, val, m->val_size);
    m->len++;
}

void* astra_map_get(const NS_Map* m, const void* key) {
    if (!m || m->len == 0) return NULL;
    char* k_bytes = (char*)m->keys;
    char* v_bytes = (char*)m->values;
    for (size_t i = 0; i < m->len; i++) {
        if (astra_keys_equal(k_bytes + i * m->key_size, key, m->key_size, m->key_is_str)) {
            return v_bytes + i * m->val_size;
        }
    }
    return NULL;
}

bool astra_map_contains(const NS_Map* m, const void* key) {
    return astra_map_get(m, key) != NULL;
}

size_t astra_map_len(const NS_Map* m) {
    return m ? m->len : 0;
}

void* astra_map_key_at(const NS_Map* m, size_t i) {
    if (!m || i >= m->len) astra_panic("map key index out of bounds");
    return (char*)m->keys + (i * m->key_size);
}

/* Signed wrapping arithmetic through unsigned round-trips to avoid C
   signed-overflow undefined behavior (design sec 6.2). */
#define ASTRA_WRAP(name, type, utype, op) \
    type name(type a, type b) { return (type)((utype)a op (utype)b); }

ASTRA_WRAP(astra_add_i8, int8_t, uint8_t, +)
ASTRA_WRAP(astra_add_i16, int16_t, uint16_t, +)
ASTRA_WRAP(astra_add_i32, int32_t, uint32_t, +)
ASTRA_WRAP(astra_add_i64, int64_t, uint64_t, +)
ASTRA_WRAP(astra_sub_i8, int8_t, uint8_t, -)
ASTRA_WRAP(astra_sub_i16, int16_t, uint16_t, -)
ASTRA_WRAP(astra_sub_i32, int32_t, uint32_t, -)
ASTRA_WRAP(astra_sub_i64, int64_t, uint64_t, -)
ASTRA_WRAP(astra_mul_i8, int8_t, uint8_t, *)
ASTRA_WRAP(astra_mul_i16, int16_t, uint16_t, *)
ASTRA_WRAP(astra_mul_i32, int32_t, uint32_t, *)
ASTRA_WRAP(astra_mul_i64, int64_t, uint64_t, *)
"#;
