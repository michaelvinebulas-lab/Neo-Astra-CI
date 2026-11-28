import test from "node:test";
import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { runCli } from "../packages/cli/dist/index.js";

async function createProject(source, ffi) {
  const directory = await mkdtemp(join(tmpdir(), "neo-astra-check-"));
  await writeFile(
    join(directory, "core.json"),
    JSON.stringify({
      package: { name: "check-demo" },
      ...(ffi === undefined ? {} : { ffi }),
    }),
    "utf8",
  );
  await mkdir(join(directory, "src"), { recursive: true });
  await writeFile(join(directory, "src", "main.ns"), source, "utf8");
  return directory;
}

async function capture(run) {
  const stdout = [];
  const stderr = [];
  const originalStdout = process.stdout.write;
  const originalStderr = process.stderr.write;
  process.stdout.write = (chunk) => {
    stdout.push(String(chunk));
    return true;
  };
  process.stderr.write = (chunk) => {
    stderr.push(String(chunk));
    return true;
  };
  try {
    const code = await run();
    return { code, stdout: stdout.join(""), stderr: stderr.join("") };
  } finally {
    process.stdout.write = originalStdout;
    process.stderr.write = originalStderr;
  }
}

const FOREIGN_SOURCE = `module app.main

foreign module native {
    fn open() -> i32
}

pub fn main() -> i32 {
    return 0
}
`;

test("astra check runs the frontend and reports a type error", async () => {
  const directory = await createProject(`module app.main
pub fn main() -> i32 {
    return "no"
}
`);
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS4001/);
    assert.match(result.stderr, /Type mismatch/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check rejects a foreign module without an ffi mapping", async () => {
  const directory = await createProject(FOREIGN_SOURCE);
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS7007/);
    assert.match(result.stderr, /Foreign module 'native'/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check accepts a foreign module with a matching ffi mapping", async () => {
  const directory = await createProject(FOREIGN_SOURCE, {
    native: {
      native: {
        source: "./libnative.so",
        type: "c-abi",
        ownership: "borrowed",
        threading: "safe",
        errors: "result-tag",
      },
    },
  });
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 0, result.stderr);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check rejects a foreign module with no mapping for the active target", async () => {
  const directory = await createProject(FOREIGN_SOURCE, {
    native: {
      typescript: { source: "./libnative.js", type: "esm" },
    },
  });
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS7007/);
    assert.match(result.stderr, /active target 'native'/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check rejects a foreign mapping without ownership metadata", async () => {
  const directory = await createProject(FOREIGN_SOURCE, {
    native: {
      native: { source: "./libnative.so", type: "c-abi" },
    },
  });
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS7007/);
    assert.match(result.stderr, /needs a non-empty ownership/);
    assert.match(result.stderr, /needs a non-empty threading/);
    assert.match(result.stderr, /needs a non-empty errors/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check rejects an inconsistent inline-script mapping", async () => {
  const source = `module app.main

{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) { return a + b }
@end
}

pub fn main() -> i32 {
    return 0
}
`;
  const directory = await createProject(source, {
    math: {
      typescript: {
        source: "./math.js",
        type: "inline-js",
        ownership: "borrowed",
        threading: "single-threaded",
        errors: "exception",
      },
    },
  });
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /NS7007/);
    assert.match(result.stderr, /source is not 'inline'/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check accepts a valid inline-script mapping", async () => {
  const source = `module app.main

{@script:js as math;
    @mode(core)
    @exports {
        fn add(a: i32, b: i32) -> i32
    }

    @impl
    export function add(a, b) { return a + b }
@end
}

pub fn main() -> i32 {
    return 0
}
`;
  const directory = await createProject(source, {
    math: {
      typescript: {
        source: "inline",
        type: "inline-js",
        ownership: "borrowed",
        threading: "single-threaded",
        errors: "exception",
      },
    },
  });
  try {
    const result = await capture(() => runCli(["check", directory], directory));
    assert.equal(result.code, 0, result.stderr);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("astra check --json reports source diagnostics and ok=false", async () => {
  const directory = await createProject(`module app.main
pub fn main() -> i32 {
    return "no"
}
`);
  try {
    const result = await capture(() => runCli(["check", directory, "--json"], directory));
    assert.equal(result.code, 1);
    const report = JSON.parse(result.stdout);
    assert.equal(report.ok, false);
    assert.equal(report.package, "check-demo");
    assert.equal(report.diagnostics[0].code, "NS4001");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
