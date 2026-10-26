//! Backend-neutral conformance cases.
//!
//! Each case is a self-contained `.ns` module with a `pub fn main() -> i32`
//! entry point plus a `.expected` file holding the exact standard output the
//! program must produce, ending in a final `RETURN=<exit code>` line. Target
//! backends compile and run every case and compare stdout byte for byte
//! (after newline normalization). See `conformance/README.md`.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const casesDirectory = fileURLToPath(
  new URL("../../conformance/cases", import.meta.url),
);

function normalize(text) {
  return text.replace(/\r\n/g, "\n").trimEnd();
}

/** Loads every conformance case as `{ name, source, expected }` records. */
export function loadConformanceCases() {
  return readdirSync(casesDirectory)
    .filter((file) => file.endsWith(".ns"))
    .sort()
    .map((file) => {
      const name = file.slice(0, -3);
      return {
        name,
        source: normalize(readFileSync(join(casesDirectory, file), "utf8")),
        expected: normalize(
          readFileSync(join(casesDirectory, `${name}.expected`), "utf8"),
        ),
      };
    });
}

export { normalize as normalizeConformanceText };
