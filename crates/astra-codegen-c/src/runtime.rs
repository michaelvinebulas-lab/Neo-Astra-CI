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

/* Neo Astra C runtime — freestanding declarations. */
#define ASTRA_NORETURN __attribute__((noreturn))

/* Managed value types shared by all generated modules. */
typedef struct { const char* data; size_t len; } NS_Str;
typedef struct { void* data; size_t len; size_t cap; } NS_Array;
typedef struct { char _unit; } NS_Unit;

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
pub const HOST_SHIM: &str = r#"/* Neo Astra C runtime — hosted implementation (astra_host.c). */
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

/* Signed wrapping arithmetic through unsigned round-trips to avoid C
   signed-overflow undefined behavior (design §6.2). */
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
