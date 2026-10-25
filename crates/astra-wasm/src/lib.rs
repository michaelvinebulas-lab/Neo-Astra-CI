//! Raw Wasm boundary for the Neo Astra compiler.
//!
//! The host writes a JSON request into linear memory, calls `astra_compile`,
//! and reads the packed response pointer/length. This avoids a mandatory
//! JavaScript runtime wrapper.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::slice;

    use astra_compiler_api::{API_VERSION_MAJOR, process_json};

    /// Returns the API major version.
    #[unsafe(no_mangle)]
    pub extern "C" fn astra_protocol_version() -> u32 {
        API_VERSION_MAJOR
    }

    /// Allocates `len` bytes in linear memory.
    #[unsafe(no_mangle)]
    pub extern "C" fn astra_alloc(len: usize) -> *mut u8 {
        let mut buffer = Vec::<u8>::with_capacity(len);
        let pointer = buffer.as_mut_ptr();
        std::mem::forget(buffer);
        pointer
    }

    /// Frees a buffer previously returned by `astra_alloc` or `astra_compile`.
    ///
    /// # Safety
    ///
    /// `pointer` and `len` must describe an allocation produced by this module.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn astra_free(pointer: *mut u8, len: usize) {
        if pointer.is_null() || len == 0 {
            return;
        }

        // SAFETY: the caller guarantees the pointer and length came from this module.
        unsafe {
            drop(Vec::from_raw_parts(pointer, 0, len));
        }
    }

    /// Compiles one JSON request and returns a packed pointer/length.
    ///
    /// The high 32 bits are the response pointer; the low 32 bits are the
    /// response length. The caller must free the response with `astra_free`.
    ///
    /// # Safety
    ///
    /// `pointer` and `len` must describe a readable request buffer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn astra_compile(pointer: *const u8, len: usize) -> u64 {
        let input = if pointer.is_null() || len == 0 {
            String::new()
        } else {
            // SAFETY: the caller guarantees a readable buffer of `len` bytes.
            let bytes = unsafe { slice::from_raw_parts(pointer, len) };
            String::from_utf8_lossy(bytes).into_owned()
        };

        let mut output = process_json(&input).into_bytes();
        output.shrink_to_fit();
        let output_len = output.len();
        let output_pointer = output.as_mut_ptr();
        std::mem::forget(output);

        ((output_pointer as u64) << 32) | output_len as u64
    }
}
