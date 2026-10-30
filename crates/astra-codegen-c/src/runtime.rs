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
pub const RUNTIME_HEADER: &str = r#"#ifndef NS_RUNTIME_HEADER
#define NS_RUNTIME_HEADER
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/* Forward declaration: GC shape descriptors (GC-001 Phase A) are referenced by
   value-type typedefs below and defined after them. */
typedef struct NS_Gc_Shape NS_Gc_Shape;

/* Neo Astra C runtime -- freestanding declarations. */
#define ASTRA_NORETURN __attribute__((noreturn))

/* Managed value types shared by all generated modules. */
typedef struct { const char* data; size_t len; } NS_Str;
typedef struct { const uint8_t* data; size_t len; } NS_Bytes;
typedef struct { void* data; size_t len; size_t cap; } NS_Array;
typedef struct { void* keys; void* values; size_t len; size_t cap; size_t key_size; size_t val_size; bool key_is_str; const NS_Gc_Shape* key_shape; const NS_Gc_Shape* val_shape; } NS_Map;
typedef struct { char _unit; } NS_Unit;
/* Built-in nominal error payload for `str.slice` (spec §6.2). Declared here
   rather than per module so every translation unit shares one stem-free type. */
typedef struct { NS_Str _0; } NS_SliceError;
/* Built-in fallible `str.to_int` and integer overflow payloads. The `_0`
   field carries the offending text, matching the tuple-struct shapes the
   checker and the other backends use. */
typedef struct { NS_Str _0; } NS_ParseIntError;
typedef struct { NS_Str _0; } NS_OverflowError;
/* Built-in `Channel` runtime (single-threaded V1 slice). `Channel<T>` maps to
   `NS_Channel*`; messages are copied by value using `elem_size`. */
typedef struct { char _unit; } NS_ChannelError;
typedef struct { void* data; size_t len; size_t cap; size_t elem_size; bool closed; } NS_Channel;
/* Built-in `Region` (spec 8.4 manual API): a fixed-capacity block carved
   bump-style. A region is a header-plus-block handle, so every pointer it
   hands out stays valid until `region.free()`. */
typedef struct { uint8_t* base; size_t cap; size_t offset; } NS_Region;

/* GC block model (GC-001 Phase A, proposal §5.5/§5.7). Every managed payload
   is preceded by an NS_Gc_Head; manual memory has no header and is never
   traced. Shapes are compiler-emitted constants describing how a payload's
   interior pointers decompose: LEAF (bytes only), ARRAY (payload is
   elem_size-strided elements described by `elem`), RECORD (payload holds
   pointer-bearing fields at the listed offsets), VARIANT (tag at tag_offset
   selects shapes[tag], NULL for a payload-less variant). Shapes compose. */
typedef enum {
    NS_SHAPE_KIND_LEAF = 0,
    NS_SHAPE_KIND_ARRAY = 1,
    NS_SHAPE_KIND_RECORD = 2,
    NS_SHAPE_KIND_VARIANT = 3
} NS_Gc_Shape_Kind;
typedef struct { size_t offset; const NS_Gc_Shape* shape; } NS_Gc_Shape_Field;
struct NS_Gc_Shape {
    NS_Gc_Shape_Kind kind;
    size_t elem_size;                 /* ARRAY: element stride */
    const NS_Gc_Shape* elem;             /* ARRAY: element shape */
    const NS_Gc_Shape_Field* fields;     /* RECORD: pointer-bearing fields */
    size_t field_count;
    size_t tag_offset;                /* VARIANT: offset of the tag */
    const NS_Gc_Shape* const* variants;  /* VARIANT: payload shape per tag */
    size_t variant_count;
};
typedef struct { uint32_t marks; size_t bytes; const NS_Gc_Shape* shape; } NS_Gc_Head;
/* Builtin shapes for the runtime's own containers (compiler-emitted programs
   add per-type shapes on top of these). */
extern const NS_Gc_Shape NS_SHAPE_LEAF;      /* bytes only */
extern const NS_Gc_Shape NS_SHAPE_STR;       /* inline NS_Str/NS_Bytes value: data -> LEAF buffer */
extern const NS_Gc_Shape NS_SHAPE_ARRAY_OF_STR; /* Array<str> buffer, used by str_chars */

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

/* Managed allocation and containers. Managed blocks carry an NS_Gc_Head and
   are recorded: unreachable blocks are reclaimed by the collector (GC-001
   Phase B; collection is off by default until Phase C), and whatever remains
   is released at exit. */
extern void* astra_gc_alloc(const NS_Gc_Shape* shape, size_t n);
/* Frees one managed block (no-op for NULL). Panics when the pointer is not a
   block this allocator handed out: the manual and managed families must not be
   crossed (proposal §5.7, spec §8.4 double-free stance). In debug builds the
   payload is poisoned with 0xDD on the way out (§8.2 use-after-free oracle). */
extern void astra_gc_free(void* p);
/* Grows or creates a managed block, zeroing the grown tail. NULL creates. */
extern void* astra_gc_realloc(void* p, const NS_Gc_Shape* shape, size_t n);
/* Manual allocation (spec §8.4): plain malloc, never recorded, never traced,
   freed only by astra_manual_free (`Memory.free`, region blocks). */
extern void* astra_manual_alloc(size_t n);
extern void astra_manual_free(void* p);
/* Manual-family accounting (mirror of the managed live/total counters). */
extern size_t astra_manual_live_count(void);
extern size_t astra_manual_total_count(void);
/* Manual-memory `Region` (spec 8.4): a fixed block carved bump-style. */
extern NS_Region* astra_region_new(size_t bytes);
extern void* astra_region_alloc(NS_Region* r, size_t n);
extern void astra_region_free(NS_Region* r);
/* Managed-lifecycle hook: releases every managed block for the run. The emitted
   main entry point calls this before returning (freestanding object compiles
   only see the declaration; the hosted shim defines it). */
extern void astra_runtime_shutdown(void);
/* GC roots and collection (GC-001 §5.1-§5.6, Phase B). One root array per
   execution context: the runtime is single-threaded and `spawn` evaluates
   inline, so there is exactly one array today. Slots hold payload pointers
   (values, not addresses) so the array may realloc while frames are live.
   `astra_gc_frame_enter` reserves `slots` zeroed entries and returns the frame
   base; `astra_gc_publish` writes a payload into an absolute slot index;
   `astra_gc_frame_exit` restores the live count before every return.
   `astra_gc_temp_enter`/`astra_gc_temp_exit` extend the same region for
   materialised call arguments (LIFO). `astra_gc_maybe_collect` is the
   safepoint hook - a no-op unless ASTRA_GC_STRESS=1, which collects there and
   inside every managed allocation; default collection stays off until the
   Phase C threshold and roots are fully exercised. */
extern size_t astra_gc_frame_enter(size_t slots);
extern void astra_gc_frame_exit(size_t rf);
extern void astra_gc_publish(size_t index, void* payload);
extern size_t astra_gc_temp_enter(void);
extern void astra_gc_temp_exit(size_t index);
extern void astra_gc_maybe_collect(void);
/* Collects now. Exposed for tests and for the stress hook itself. */
extern void astra_gc_collect(void);
extern size_t astra_gc_collected_count(void);
extern size_t astra_gc_root_count(void);
/* Boundary pins (GC-001 §5.8): a managed payload returned from an exported
   symbol is pinned for the rest of the run, so the `11-c-abi.md` §5 promise
   ("valid until the process exits") keeps holding once collection is on.
   Pinning is monotone (release is post-V1); NULL is ignored. */
extern void astra_gc_pin(void* payload);
extern size_t astra_gc_pin_count(void);
extern NS_Array astra_array_new_lit(size_t n, size_t elem_size, const void* src, const NS_Gc_Shape* shape);
extern void astra_array_push(NS_Array* arr, size_t elem_size, const void* value, const NS_Gc_Shape* shape);
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
/* UTF-8 scalar helpers for `str.char_count`, `char_at`, `bytes`, and `chars`. */
extern size_t astra_str_char_count(NS_Str s);
extern bool astra_str_char_at(NS_Str s, size_t index, NS_Str* out);
extern NS_Array astra_str_bytes(NS_Str s);
extern NS_Array astra_str_chars(NS_Str s);

/* Bytes helpers. */
extern bool astra_bytes_eq(NS_Bytes a, NS_Bytes b);
extern int astra_bytes_cmp(NS_Bytes a, NS_Bytes b);
extern uint8_t astra_bytes_get(NS_Bytes b, size_t i);
/* Bounds-checked byte write (`buf[i] = v`). */
extern void astra_bytes_set(NS_Bytes* b, size_t i, uint8_t value);
/* Zero-filled allocation for `bytes.new(length)`. */
extern NS_Bytes astra_bytes_new(size_t len);
/* Infallible byte-offset slice for `bytes.slice(start, end)`; bounds-checked,
   so a reversed or out-of-range panics. */
extern NS_Bytes astra_bytes_slice(NS_Bytes b, size_t start, size_t end);

/* Map helpers. The key/value buffer shapes are stored so growth can re-head
   the buffers without the caller (GC-001 Phase A). */
extern NS_Map astra_map_new(size_t key_size, size_t val_size, bool key_is_str, const NS_Gc_Shape* key_shape, const NS_Gc_Shape* val_shape);
extern void astra_map_set(NS_Map* m, const void* key, const void* val);
extern void* astra_map_get(const NS_Map* m, const void* key);
extern bool astra_map_contains(const NS_Map* m, const void* key);
extern size_t astra_map_len(const NS_Map* m);
extern void* astra_map_key_at(const NS_Map* m, size_t i);

/* Bounded single-threaded channels (CONC-001 first slice). `send` fails on a
   closed channel and panics on a full open channel; `receive` fails on a closed
   and empty channel and panics on an empty open channel (the JS/Nova deadlock
   rule). Buffered messages remain readable after `close`. */
/* handle_shape describes the NS_Channel handle block (its data field points at
   the buffer); data_shape describes the message buffer. */
extern NS_Channel* astra_channel_new(size_t elem_size, const NS_Gc_Shape* handle_shape, const NS_Gc_Shape* data_shape);
extern bool astra_channel_send(NS_Channel* c, const void* value);
extern bool astra_channel_receive(NS_Channel* c, void* out);
extern void astra_channel_close(NS_Channel* c);

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
#endif /* NS_RUNTIME_HEADER */
"#;

/// Body of the hosted runtime implementation.
///
/// This is not a complete translation unit: it assumes [`RUNTIME_HEADER`] is in
/// scope for `NS_*` value types and the `extern` prototypes. [`host_shim`]
/// returns the complete `astra_host.c` artifact.
pub const HOST_SHIM: &str = r#"/* Neo Astra C runtime -- hosted implementation (astra_host.c).
   Shared value types and runtime prototypes come from RUNTIME_HEADER, which
   `host_shim()` prepends. Only libc headers belong in this file. */
#include <inttypes.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

/* GC block model (GC-001 Phase A). Builtin shapes for the runtime's own
   containers; emitted programs add per-type shapes on top of these. */
static const NS_Gc_Shape_Field astra_shape_fields_str[] = { { 0, &NS_SHAPE_LEAF } };
const NS_Gc_Shape NS_SHAPE_LEAF = { .kind = NS_SHAPE_KIND_LEAF };
const NS_Gc_Shape NS_SHAPE_STR = { .kind = NS_SHAPE_KIND_RECORD, .fields = astra_shape_fields_str, .field_count = 1 };
const NS_Gc_Shape NS_SHAPE_ARRAY_OF_STR = { .kind = NS_SHAPE_KIND_ARRAY, .elem_size = sizeof(NS_Str), .elem = &NS_SHAPE_STR };

/* Managed blocks: every `astra_gc_alloc` payload is recorded, with its
   NS_Gc_Head immediately before it, so the whole run can be released at exit.
   This is the V1 "arena per program run" managed-memory first step from
   03-runtime-and-memory.md §4 - now headered and shaped, so later GC-001
   phases turn the same bookkeeping into collection instead of replacing it.
   Individual blocks are reclaimed by `astra_gc_free` where the backend knows
   the old buffer is dead (e.g. array growth), and `astra_runtime_shutdown`
   frees whatever remains. Manual memory (`astra_manual_alloc`) is a separate
   family: not recorded, never traced, released only by `astra_manual_free`
   (proposal §5.7, spec §8.4). */
typedef struct NS_Arena_Node { void* ptr; struct NS_Arena_Node* next; } NS_Arena_Node;
static NS_Arena_Node* astra_arena_head = NULL;
static size_t astra_arena_live = 0;
static size_t astra_arena_total = 0;
static size_t astra_manual_live = 0;
static size_t astra_manual_total = 0;

/* GC roots and collection (GC-001 §5.1-§5.6). One root array per execution
   context: the runtime is single-threaded and `spawn` evaluates inline, so
   there is exactly one array today; CONC-001 makes it one per task. Slots hold
   payload pointers (values, not addresses), so the array may realloc while
   frames are live. Frames reserve zeroed slots and restore the count before
   every return; `astra_gc_temp_enter`/`exit` extend the same region for
   materialised call arguments. */
static void** astra_roots = NULL;
static size_t astra_roots_cap = 0;
static size_t astra_root_count = 0;
/* Permanent root set for values returned across the C ABI (§5.8). Unlike a
   frame slot, a pin is never released until shutdown; the set is monotone by
   design for V1. */
static void** astra_gc_pins = NULL;
static size_t astra_gc_pins_cap = 0;
static size_t astra_gc_pins_len = 0;
static int astra_gc_stress = -1;
static size_t astra_gc_collections = 0;

static void astra_gc_init_stress(void) {
    if (astra_gc_stress >= 0) { return; }
    const char* value = getenv("ASTRA_GC_STRESS");
    astra_gc_stress = (value && value[0] == '1' && value[1] == '\0') ? 1 : 0;
}

static void astra_gc_reserve(size_t need) {
    if (need <= astra_roots_cap) { return; }
    size_t cap = astra_roots_cap == 0 ? 64 : astra_roots_cap;
    while (cap < need) { cap *= 2; }
    void** fresh = (void**)realloc(astra_roots, cap * sizeof(void*));
    if (!fresh) { astra_panic("out of memory"); }
    astra_roots = fresh;
    astra_roots_cap = cap;
}

size_t astra_gc_frame_enter(size_t slots) {
    astra_gc_init_stress();
    size_t rf = astra_root_count;
    astra_gc_reserve(rf + slots);
    memset(astra_roots + rf, 0, slots * sizeof(void*));
    astra_root_count = rf + slots;
    return rf;
}

void astra_gc_frame_exit(size_t rf) {
    if (rf < astra_root_count) { astra_root_count = rf; }
}

void astra_gc_publish(size_t index, void* payload) {
    // Slots are assigned per declaration site, so a declaring statement inside
    // a loop re-publishes the same index each iteration. Growing on demand is
    // therefore idempotent and keeps the frame bound independent of any static
    // pre-pass over the body.
    if (index >= astra_root_count) {
        astra_gc_reserve(index + 1);
        memset(
            astra_roots + astra_root_count,
            0,
            (index + 1 - astra_root_count) * sizeof(void*)
        );
        astra_root_count = index + 1;
    }
    astra_roots[index] = payload;
}

size_t astra_gc_temp_enter(void) {
    size_t index = astra_root_count;
    astra_gc_reserve(index + 1);
    astra_roots[index] = NULL;
    astra_root_count = index + 1;
    return index;
}

void astra_gc_temp_exit(size_t index) {
    if (index < astra_root_count) { astra_root_count = index; }
}

size_t astra_gc_root_count(void) { return astra_root_count; }
size_t astra_gc_collected_count(void) { return astra_gc_collections; }

void astra_gc_pin(void* payload) {
    if (!payload) { return; }
    if (astra_gc_pins_len == astra_gc_pins_cap) {
        size_t cap = astra_gc_pins_cap == 0 ? 32 : astra_gc_pins_cap * 2;
        void** fresh = (void**)realloc(astra_gc_pins, cap * sizeof(void*));
        if (!fresh) { astra_panic("out of memory"); }
        astra_gc_pins = fresh;
        astra_gc_pins_cap = cap;
    }
    astra_gc_pins[astra_gc_pins_len++] = payload;
}

size_t astra_gc_pin_count(void) { return astra_gc_pins_len; }

static NS_Gc_Head* astra_gc_head(void* payload) {
    return ((NS_Gc_Head*)payload) - 1;
}

/* Use-after-free oracle (GC-001 §8.2): in debug builds a freed payload is
   filled with 0xDD, so a stale read of a collected block shows the poison
   pattern instead of plausible-looking data. Release builds (#define NDEBUG)
   skip the fill. Manual blocks are not poisoned: they carry no size header,
   and spec §8.4 already leaves their use-after-free undefined. */
static void astra_gc_poison(NS_Gc_Head* head) {
#ifndef NDEBUG
    memset((unsigned char*)(head + 1), 0xDD, head->bytes);
#else
    (void)head;
#endif
}

void* astra_gc_alloc(const NS_Gc_Shape* shape, size_t n) {
    /* Under stress, collect before the new block exists: everything the caller
       still needs must already be published (§5.4 amendment, §8.1). */
    astra_gc_init_stress();
    if (astra_gc_stress) { astra_gc_collect(); }
    size_t bytes = sizeof(NS_Gc_Head) + (n == 0 ? 1 : n);
    NS_Gc_Head* head = (NS_Gc_Head*)calloc(bytes, 1);
    if (!head) { astra_panic("out of memory"); }
    head->marks = 0;
    head->bytes = n;
    head->shape = shape;
    NS_Arena_Node* node = (NS_Arena_Node*)malloc(sizeof(NS_Arena_Node));
    if (!node) { free(head); astra_panic("out of memory"); }
    node->ptr = (void*)(head + 1);
    node->next = astra_arena_head;
    astra_arena_head = node;
    astra_arena_live += 1;
    astra_arena_total += 1;
    return (void*)(head + 1);
}

/* Frees one managed block (no-op for NULL). A pointer that is not a tracked
   payload is a family crossing (or a double free) and panics; manual memory is
   freed through `astra_manual_free`. */
void astra_gc_free(void* p) {
    if (!p) { return; }
    NS_Arena_Node** cur = &astra_arena_head;
    while (*cur) {
        if ((*cur)->ptr == p) {
            NS_Arena_Node* dead = *cur;
            *cur = dead->next;
            astra_gc_poison(astra_gc_head(dead->ptr));
            free(astra_gc_head(dead->ptr));
            free(dead);
            astra_arena_live -= 1;
            return;
        }
        cur = &(*cur)->next;
    }
    astra_panic("free of a pointer that is not a managed block");
}

/* Grows or creates a managed block. The grown tail is zeroed so spare
   capacity never reads as pointers (the [P5] amendment's zero-on-alloc rule,
   applied to growth). The shape argument only matters for a NULL create; a
   tracked block keeps the shape it was allocated with. */
void* astra_gc_realloc(void* p, const NS_Gc_Shape* shape, size_t n) {
    if (!p) { return astra_gc_alloc(shape, n); }
    NS_Gc_Head* head = astra_gc_head(p);
    size_t old_bytes = head->bytes;
    NS_Gc_Head* fresh = (NS_Gc_Head*)realloc(head, sizeof(NS_Gc_Head) + (n == 0 ? 1 : n));
    if (!fresh) { astra_panic("out of memory"); }
    fresh->bytes = n;
    if (n > old_bytes) {
        memset(((unsigned char*)(fresh + 1)) + old_bytes, 0, n - old_bytes);
    }
    void* payload = (void*)(fresh + 1);
    NS_Arena_Node** cur = &astra_arena_head;
    while (*cur) {
        if ((*cur)->ptr == p) {
            (*cur)->ptr = payload;
            return payload;
        }
        cur = &(*cur)->next;
    }
    astra_panic("realloc of a pointer that is not a managed block");
}

void* astra_manual_alloc(size_t n) {
    void* p = malloc(n == 0 ? 1 : n);
    if (!p) { astra_panic("out of memory"); }
    astra_manual_live += 1;
    astra_manual_total += 1;
    return p;
}

void astra_manual_free(void* p) {
    if (!p) { return; }
    astra_manual_live -= 1;
    free(p);
}

/* Directory of live blocks, rebuilt and sorted once per collection so an
   interior pointer (a `str`/`bytes` slice, spec §5.6) resolves to its block by
   containment instead of by exact base match. */
typedef struct { void* payload; size_t bytes; } NS_Gc_Dir_Entry;
static NS_Gc_Dir_Entry* astra_gc_dir = NULL;
static size_t astra_gc_dir_count = 0;

static int astra_gc_dir_cmp(const void* a, const void* b) {
    uintptr_t x = (uintptr_t)((const NS_Gc_Dir_Entry*)a)->payload;
    uintptr_t y = (uintptr_t)((const NS_Gc_Dir_Entry*)b)->payload;
    return x < y ? -1 : (x > y ? 1 : 0);
}

/* Mark stack: explicit, so deep object graphs cannot overflow the C stack. */
static void** astra_mark_stack = NULL;
static size_t astra_mark_len = 0;
static size_t astra_mark_cap = 0;

static void astra_gc_mark_push(void* payload) {
    NS_Gc_Head* head = astra_gc_head(payload);
    if (head->marks != 0) { return; }
    head->marks = 1;
    if (astra_mark_len == astra_mark_cap) {
        size_t cap = astra_mark_cap == 0 ? 64 : astra_mark_cap * 2;
        void** fresh = (void**)realloc(astra_mark_stack, cap * sizeof(void*));
        if (!fresh) { astra_panic("out of memory"); }
        astra_mark_stack = fresh;
        astra_mark_cap = cap;
    }
    astra_mark_stack[astra_mark_len++] = payload;
}

/* Resolves a slot value to its block: an exact base pointer, or the block that
   contains the interior pointer (slices). A value outside every live block is
   ignored: only real pointers are followed (precise roots, §4.1). */
static void astra_gc_mark_pointer(void* value) {
    if (!value) { return; }
    size_t lo = 0;
    size_t hi = astra_gc_dir_count;
    while (lo < hi) {
        size_t mid = lo + (hi - lo) / 2;
        if ((uintptr_t)astra_gc_dir[mid].payload <= (uintptr_t)value) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if (lo == 0) { return; }
    const NS_Gc_Dir_Entry* entry = &astra_gc_dir[lo - 1];
    uintptr_t base = (uintptr_t)entry->payload;
    uintptr_t pointer = (uintptr_t)value;
    if (pointer == base || pointer < base + entry->bytes) {
        astra_gc_mark_push(entry->payload);
    }
}

static void astra_gc_trace(const NS_Gc_Shape* shape, unsigned char* base, size_t bytes) {
    if (!shape) { return; }
    switch (shape->kind) {
    case NS_SHAPE_KIND_LEAF:
        break;
    case NS_SHAPE_KIND_ARRAY: {
        if (shape->elem_size == 0) { break; }
        size_t elements = bytes / shape->elem_size;
        for (size_t i = 0; i < elements; i++) {
            astra_gc_trace(shape->elem, base + i * shape->elem_size, 0);
        }
        break;
    }
    case NS_SHAPE_KIND_RECORD:
        for (size_t i = 0; i < shape->field_count; i++) {
            void* slot = *(void**)(base + shape->fields[i].offset);
            astra_gc_mark_pointer(slot);
        }
        break;
    case NS_SHAPE_KIND_VARIANT: {
        if (shape->variant_count == 0) { break; }
        int tag = *(const int*)(base + shape->tag_offset);
        if (tag >= 0 && (size_t)tag < shape->variant_count && shape->variants[tag]) {
            astra_gc_trace(shape->variants[tag], base, 0);
        }
        break;
    }
    }
}

void astra_gc_collect(void) {
    size_t count = astra_arena_live;
    NS_Gc_Dir_Entry* dir = NULL;
    if (count > 0) {
        dir = (NS_Gc_Dir_Entry*)malloc(count * sizeof(NS_Gc_Dir_Entry));
        if (!dir) { astra_panic("out of memory"); }
        size_t i = 0;
        for (NS_Arena_Node* node = astra_arena_head; node; node = node->next) {
            dir[i].payload = node->ptr;
            dir[i].bytes = astra_gc_head(node->ptr)->bytes;
            i++;
        }
        qsort(dir, count, sizeof(NS_Gc_Dir_Entry), astra_gc_dir_cmp);
    }
    astra_gc_dir = dir;
    astra_gc_dir_count = count;
    /* Mark: roots, then pinned boundary values, then everything reachable
       through the shapes. */
    for (size_t i = 0; i < astra_root_count; i++) {
        astra_gc_mark_pointer(astra_roots[i]);
    }
    for (size_t i = 0; i < astra_gc_pins_len; i++) {
        astra_gc_mark_pointer(astra_gc_pins[i]);
    }
    while (astra_mark_len > 0) {
        void* payload = astra_mark_stack[--astra_mark_len];
        NS_Gc_Head* head = astra_gc_head(payload);
        astra_gc_trace(head->shape, (unsigned char*)payload, head->bytes);
    }
    /* Sweep: unmarked blocks are garbage; survivors keep their storage and
       lose the mark for the next collection. */
    NS_Arena_Node** cur = &astra_arena_head;
    while (*cur) {
        NS_Arena_Node* node = *cur;
        NS_Gc_Head* head = astra_gc_head(node->ptr);
        if (head->marks == 0) {
            *cur = node->next;
            astra_gc_poison(head);
            free(head);
            free(node);
            astra_arena_live -= 1;
        } else {
            head->marks = 0;
            cur = &node->next;
        }
    }
    astra_gc_dir = NULL;
    astra_gc_dir_count = 0;
    free(dir);
    astra_gc_collections += 1;
}

/* Safepoint hook: default off until Phase C turns collection on by default.
   ASTRA_GC_STRESS=1 collects here and inside every managed allocation, so the
   whole gate can prove the root discipline (§5.4/§8.1). */
void astra_gc_maybe_collect(void) {
    astra_gc_init_stress();
    if (astra_gc_stress) { astra_gc_collect(); }
}

/* `Region` (spec 8.4): a fixed-capacity manual block carved bump-style.
   `Region.new(bytes)` reserves one block; `region.alloc<T>(n)` hands out the
   next `n * sizeof(T)` bytes and panics when the block is exhausted (the spec
   types this `! AllocError`; until the fallible entry-point policy lands,
   allocation failure is a panic, matching `astra_gc_alloc`). Every carv is
   aligned to 8 bytes so sequential allocations of any scalar or struct the
   language can form stay naturally aligned. `region.free()` returns the whole
   block and zeroes the header, so a double-free is a no-op rather than a
   crash (spec 8.4 makes double-free a programmer bug). Regions never move, so
   every pointer handed out stays valid until the region is freed. The handle
   is a managed LEAF block (its `base` points at manual memory, which the
   collector never traces); the carved block itself is manual (proposal §5.7). */
#define ASTRA_REGION_ALIGN 8

NS_Region* astra_region_new(size_t bytes) {
    NS_Region* r = (NS_Region*)astra_gc_alloc(&NS_SHAPE_LEAF, sizeof(NS_Region));
    r->base = (uint8_t*)astra_manual_alloc(bytes == 0 ? 1 : bytes);
    r->cap = bytes;
    r->offset = 0;
    return r;
}

void* astra_region_alloc(NS_Region* r, size_t n) {
    if (!r || !r->base) { astra_panic("allocation from a freed region"); }
    size_t start = (r->offset + (ASTRA_REGION_ALIGN - 1)) & ~(size_t)(ASTRA_REGION_ALIGN - 1);
    if (n > r->cap || start > r->cap - n) { astra_panic("region capacity exceeded"); }
    r->offset = start + n;
    return r->base + start;
}

void astra_region_free(NS_Region* r) {
    if (!r || !r->base) { return; }
    astra_manual_free(r->base);
    r->base = NULL;
    r->cap = 0;
    r->offset = 0;
}

void astra_runtime_shutdown(void) {
    NS_Arena_Node* cur = astra_arena_head;
    while (cur) {
        NS_Arena_Node* next = cur->next;
        free(astra_gc_head(cur->ptr));
        free(cur);
        cur = next;
    }
    astra_arena_head = NULL;
    astra_arena_live = 0;
    /* The root array and mark stack are plain libc memory, not managed blocks;
       release them here so a long-lived host that runs several programs does
       not keep the last program's bookkeeping. */
    free(astra_roots);
    astra_roots = NULL;
    astra_roots_cap = 0;
    astra_root_count = 0;
    free(astra_gc_pins);
    astra_gc_pins = NULL;
    astra_gc_pins_cap = 0;
    astra_gc_pins_len = 0;
    free(astra_mark_stack);
    astra_mark_stack = NULL;
    astra_mark_len = 0;
    astra_mark_cap = 0;
}

/* Test/observability accessors for both families (leak checks in the native
   gate). The manual counters reflect astra_manual_alloc/free balance; the
   managed counters reflect tracked gc blocks, released wholesale at shutdown. */
size_t astra_arena_live_count(void) { return astra_arena_live; }
size_t astra_arena_total_count(void) { return astra_arena_total; }
size_t astra_manual_live_count(void) { return astra_manual_live; }
size_t astra_manual_total_count(void) { return astra_manual_total; }

NS_Array astra_array_new_lit(size_t n, size_t elem_size, const void* src, const NS_Gc_Shape* shape) {
    NS_Array arr;
    arr.cap = n == 0 ? 1 : n;
    arr.len = n;
    arr.data = astra_gc_alloc(shape, arr.cap * (elem_size == 0 ? 1 : elem_size));
    if (n > 0) { memcpy(arr.data, src, n * elem_size); }
    return arr;
}

/* Growth reuses the buffer's own header shape when a block already exists;
   the passed shape covers a from-empty start (e.g. `str_chars`). */
void astra_array_push(NS_Array* arr, size_t elem_size, const void* value, const NS_Gc_Shape* shape) {
    if (arr->len >= arr->cap) {
        size_t new_cap = arr->cap == 0 ? 4 : arr->cap * 2;
        arr->data = astra_gc_realloc(arr->data, shape, new_cap * (elem_size == 0 ? 1 : elem_size));
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

/* Width in bytes of the UTF-8 scalar that starts at `data`, or 0 when the
   leading byte is not a scalar start or the sequence is truncated. */
static size_t astra_utf8_width(const char* data, size_t len) {
    unsigned char lead = (unsigned char)data[0];
    if (lead < 0x80) return 1;
    size_t width = lead >= 0xF0 ? 4 : lead >= 0xE0 ? 3 : lead >= 0xC0 ? 2 : 0;
    if (width == 0 || width > len) return 0;
    return width;
}

size_t astra_str_char_count(NS_Str s) {
    size_t count = 0;
    size_t i = 0;
    while (i < s.len) {
        size_t width = astra_utf8_width(s.data + i, s.len - i);
        if (width == 0) width = 1;
        count += 1;
        i += width;
    }
    return count;
}

bool astra_str_char_at(NS_Str s, size_t index, NS_Str* out) {
    size_t count = 0;
    size_t i = 0;
    while (i < s.len) {
        size_t width = astra_utf8_width(s.data + i, s.len - i);
        if (width == 0) width = 1;
        if (count == index) {
            out->data = s.data + i;
            out->len = width;
            return true;
        }
        count += 1;
        i += width;
    }
    return false;
}

NS_Array astra_str_bytes(NS_Str s) {
    NS_Array arr;
    arr.data = astra_gc_alloc(&NS_SHAPE_LEAF, s.len == 0 ? 1 : s.len);
    arr.len = s.len;
    arr.cap = s.len;
    if (s.len > 0) memcpy(arr.data, s.data, s.len);
    return arr;
}

NS_Array astra_str_chars(NS_Str s) {
    NS_Array arr;
    arr.data = NULL;
    arr.len = 0;
    arr.cap = 0;
    size_t i = 0;
    while (i < s.len) {
        size_t width = astra_utf8_width(s.data + i, s.len - i);
        if (width == 0) width = 1;
        NS_Str part;
        part.data = s.data + i;
        part.len = width;
        astra_array_push(&arr, sizeof(NS_Str), &part, &NS_SHAPE_ARRAY_OF_STR);
        i += width;
    }
    return arr;
}

NS_Bytes astra_bytes_new(size_t len) {
    NS_Bytes b;
    /* A managed LEAF block: tracked like every other buffer, and zeroed by
       gc_alloc (calloc) exactly as the old calloc did. */
    b.data = (const uint8_t*)astra_gc_alloc(&NS_SHAPE_LEAF, len == 0 ? 1 : len);
    if (b.data == NULL) astra_panic("out of memory");
    b.len = len;
    return b;
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

void astra_bytes_set(NS_Bytes* b, size_t i, uint8_t value) {
    if (i >= b->len) astra_panic("byte index out of bounds");
    ((uint8_t*)b->data)[i] = value;
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

NS_Map astra_map_new(size_t key_size, size_t val_size, bool key_is_str, const NS_Gc_Shape* key_shape, const NS_Gc_Shape* val_shape) {
    NS_Map m;
    m.keys = NULL;
    m.values = NULL;
    m.len = 0;
    m.cap = 0;
    m.key_size = key_size;
    m.val_size = val_size;
    m.key_is_str = key_is_str;
    m.key_shape = key_shape;
    m.val_shape = val_shape;
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
        /* Two managed blocks grow here. The stress hook collects inside every
           managed allocation, so the freshly grown key buffer must be rooted
           while the value buffer is allocated - the map's own root slots still
           hold the pre-growth pointers (the call has not returned). */
        size_t root = astra_gc_temp_enter();
        m->keys = astra_gc_realloc(m->keys, m->key_shape, new_cap * (m->key_size == 0 ? 1 : m->key_size));
        astra_gc_publish(root, m->keys);
        m->values = astra_gc_realloc(m->values, m->val_shape, new_cap * (m->val_size == 0 ? 1 : m->val_size));
        astra_gc_temp_exit(root);
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

/* Bounded single-threaded channels (CONC-001 first slice). The V1 portable
   runtime is cooperative, so blocking is a structured deadlock panic. */
#define ASTRA_CHANNEL_CAP 16

NS_Channel* astra_channel_new(size_t elem_size, const NS_Gc_Shape* handle_shape, const NS_Gc_Shape* data_shape) {
    /* Two managed blocks. The stress hook collects inside every managed
       allocation, so the handle must be rooted while the data buffer is
       allocated - otherwise the second allocation would free the handle, which
       the language cannot reference yet. */
    size_t root = astra_gc_temp_enter();
    NS_Channel* c = (NS_Channel*)astra_gc_alloc(handle_shape, sizeof(NS_Channel));
    astra_gc_publish(root, c);
    c->data = astra_gc_alloc(data_shape, elem_size * ASTRA_CHANNEL_CAP);
    astra_gc_temp_exit(root);
    c->len = 0;
    c->cap = ASTRA_CHANNEL_CAP;
    c->elem_size = elem_size;
    c->closed = false;
    return c;
}

bool astra_channel_send(NS_Channel* c, const void* value) {
    if (!c || c->closed) return false;
    if (c->len >= c->cap) astra_panic("channel deadlock: send on a full channel");
    memcpy((char*)c->data + (c->len * c->elem_size), value, c->elem_size);
    c->len++;
    return true;
}

bool astra_channel_receive(NS_Channel* c, void* out) {
    if (!c) return false;
    if (c->len == 0) {
        if (c->closed) return false;
        astra_panic("channel deadlock: receive on an empty channel");
    }
    memcpy(out, c->data, c->elem_size);
    memmove(c->data, (char*)c->data + c->elem_size, (c->len - 1) * c->elem_size);
    c->len--;
    return true;
}

void astra_channel_close(NS_Channel* c) {
    if (c) c->closed = true;
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

/// Complete `astra_host.c` translation unit.
///
/// The shim is compiled as its own translation unit, so it must see the shared
/// runtime declarations ([`RUNTIME_HEADER`]) — `NS_Bytes`, `NS_Map`, and the
/// builtin error payload typedefs are used by the hosted implementation but are
/// not declared in [`HOST_SHIM`] itself. Prepending the header keeps one
/// definition of every `NS_*` type instead of duplicating (and drifting) them.
pub fn host_shim() -> String {
    format!("{RUNTIME_HEADER}\n{HOST_SHIM}")
}
