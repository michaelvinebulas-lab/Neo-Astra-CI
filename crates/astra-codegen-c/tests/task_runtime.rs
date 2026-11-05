//! Runtime-level tests for the native task primitives (CONC-001 A2/C2).
//!
//! These tests do not go through the emitter yet: they compile a small
//! hand-written program together with [`astra_codegen_c::runtime::host_shim`]
//! and run it, so the thread, per-task heap, and cross-heap copy primitives
//! are verified before `spawn` lowers to them.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tool_path(name: &str) -> Option<PathBuf> {
    if Path::new(name).is_absolute() {
        return Path::new(name).is_file().then(|| PathBuf::from(name));
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in ["", ".exe", ".cmd", ".bat"] {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn hosted_probe(compiler: &Path) -> bool {
    let Ok(dir) = tempfile::tempdir() else {
        return false;
    };
    let probe = dir.path().join("probe.c");
    let obj = dir.path().join("probe.o");
    if std::fs::write(&probe, "#include <stdio.h>\nint main(void) { return 0; }\n").is_err() {
        return false;
    }
    Command::new(compiler)
        .args(["-std=c17", "-c", "-o"])
        .arg(&obj)
        .arg(&probe)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn hosted_compiler() -> Option<PathBuf> {
    for candidate in [
        "C:\\Program Files\\LLVM\\bin\\clang.exe",
        "clang",
        "gcc",
        "cc",
    ] {
        if let Some(path) = tool_path(candidate)
            && hosted_probe(&path)
        {
            return Some(path);
        }
    }
    None
}

/// Compiles `program` with the host shim and runs it. Returns `None` when no
/// hosted compiler is available (matching the other hosted tests' self-skip).
fn build_and_run(program: &str, stress: bool) -> Option<(bool, String, String)> {
    let compiler = hosted_compiler()?;
    let dir = tempfile::tempdir().ok()?;
    let source = dir.path().join("task_runtime.c");
    let binary = dir
        .path()
        .join(format!("task_runtime{}", std::env::consts::EXE_SUFFIX));
    let mut text = astra_codegen_c::runtime::host_shim();
    text.push('\n');
    text.push_str(program);
    std::fs::write(&source, text).ok()?;
    let compiled = Command::new(&compiler)
        .args(["-std=c17", "-O0", "-g", "-o"])
        .arg(&binary)
        .arg(&source)
        .output()
        .ok()?;
    if !compiled.status.success() {
        return Some((
            false,
            String::new(),
            String::from_utf8_lossy(&compiled.stderr).into_owned(),
        ));
    }
    let mut command = Command::new(&binary);
    if stress {
        command.env("ASTRA_GC_STRESS", "1");
    }
    let run = command.output().ok()?;
    Some((
        run.status.success(),
        String::from_utf8_lossy(&run.stdout).into_owned(),
        String::from_utf8_lossy(&run.stderr).into_owned(),
    ))
}

const THREADS_AND_JOIN: &str = r#"
typedef struct { NS_Task_Core core; int value; } NS_Task_int;

static void body(void* arg) {
    NS_Task_int* task = (NS_Task_int*)arg;
    task->value = 21 + 21;
}

int main(void) {
    NS_Task_int* task = (NS_Task_int*)astra_gc_alloc(&NS_SHAPE_LEAF, sizeof(NS_Task_int));
    astra_gc_root_in(astra_heap(), task);
    task->core.entry = body;
    task->core.closure = task;
    astra_task_start(&task->core);
    if (!astra_task_join(&task->core)) { astra_task_raise(&task->core); }
    /* Cancellation after completion is a no-op and is not recorded. */
    astra_task_cancel(&task->core);
    printf("%d %d\n", task->value, task->core.canceled ? 1 : 0);
    astra_task_heap_free(&task->core);
    astra_runtime_shutdown();
    return 0;
}
"#;

const CAPTURE_AND_RESULT_COPY: &str = r#"
typedef struct { NS_Task_Core core; NS_Str value; } NS_Task_str;
typedef struct { NS_Task_str* task; NS_Str captured; NS_Str value; } NS_Closure;

static const NS_Gc_Shape_Field closure_fields[] = {
    { offsetof(NS_Closure, captured), &NS_SHAPE_STR },
    { offsetof(NS_Closure, value), &NS_SHAPE_STR },
};
static const NS_Gc_Shape NS_SHAPE_CLOSURE = {
    .kind = NS_SHAPE_KIND_RECORD,
    .fields = closure_fields,
    .field_count = 2,
};

static void body(void* arg) {
    NS_Closure* closure = (NS_Closure*)arg;
    char* data = (char*)astra_gc_alloc(&NS_SHAPE_LEAF, closure->captured.len + 1);
    memcpy(data, closure->captured.data, closure->captured.len);
    data[closure->captured.len] = '\0';
    closure->value.data = data;
    closure->value.len = closure->captured.len;
}

int main(void) {
    NS_Str parent = {0};
    const char* text = "hello";
    char* data = (char*)astra_gc_alloc(&NS_SHAPE_LEAF, 6);
    memcpy(data, text, 6);
    astra_gc_root_in(astra_heap(), data);
    parent.data = data;
    parent.len = 5;

    NS_Task_str* task = (NS_Task_str*)astra_gc_alloc(&NS_SHAPE_LEAF, sizeof(NS_Task_str));
    astra_gc_root_in(astra_heap(), task);
    task->core.heap = astra_heap_create();
    NS_Closure* closure = (NS_Closure*)astra_gc_alloc_in_heap(
        task->core.heap, &NS_SHAPE_CLOSURE, sizeof(NS_Closure));
    astra_gc_root_in(task->core.heap, closure);
    closure->task = task;
    astra_gc_copy_into(task->core.heap, astra_heap(), &NS_SHAPE_STR,
        sizeof(NS_Str), &closure->captured, &parent);

    task->core.entry = body;
    task->core.closure = closure;
    astra_task_start(&task->core);
    if (!astra_task_join(&task->core)) { astra_task_raise(&task->core); }

    astra_gc_copy_into(astra_heap(), task->core.heap, &NS_SHAPE_STR,
        sizeof(NS_Str), &task->value, &closure->value);
    astra_task_heap_free(&task->core);

    printf("%.*s %.*s\n",
        (int)parent.len, parent.data,
        (int)task->value.len, task->value.data);
    astra_runtime_shutdown();
    return 0;
}
"#;

#[test]
fn a_thread_runs_and_joins() {
    let Some((ok, stdout, stderr)) = build_and_run(THREADS_AND_JOIN, false) else {
        return;
    };
    assert!(ok, "execution failed:\nstderr:\n{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["42 0"]);
}

#[test]
fn a_capture_and_result_survive_the_child_heap() {
    let Some((ok, stdout, stderr)) = build_and_run(CAPTURE_AND_RESULT_COPY, true) else {
        return;
    };
    assert!(ok, "execution failed:\nstderr:\n{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["hello hello"]);
}

const JOINED_PANIC_BECOMES_TASK_ERROR: &str = r#"
typedef struct { NS_Task_Core core; int value; } NS_Task_int;

static void body(void* arg) {
    (void)arg;
    astra_panic("boom");
}

int main(void) {
    NS_Task_int* task = (NS_Task_int*)astra_gc_alloc(&NS_SHAPE_LEAF, sizeof(NS_Task_int));
    astra_gc_root_in(astra_heap(), task);
    task->core.entry = body;
    task->core.closure = task;
    astra_task_start(&task->core);
    printf("before\n");
    fflush(stdout);
    if (!astra_task_join(&task->core)) { astra_task_raise(&task->core); }
    printf("after\n");
    astra_task_heap_free(&task->core);
    astra_runtime_shutdown();
    return 0;
}
"#;

#[test]
fn a_child_panic_is_raised_as_a_task_error_at_join() {
    let Some((ok, stdout, stderr)) = build_and_run(JOINED_PANIC_BECOMES_TASK_ERROR, false) else {
        return;
    };
    assert!(!ok, "a joined child panic must fail the parent task");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["before"]);
    assert!(
        stderr.contains("panic: TaskError: boom"),
        "expected a structured TaskError, got:\n{stderr}"
    );
}

const CANCELLED_TASK_BECOMES_TASK_ERROR: &str = r#"
typedef struct { NS_Task_Core core; int value; } NS_Task_int;

static void body(void* arg) {
    (void)arg;
    for (;;) { astra_task_check(); }
}

int main(void) {
    NS_Task_int* task = (NS_Task_int*)astra_gc_alloc(&NS_SHAPE_LEAF, sizeof(NS_Task_int));
    astra_gc_root_in(astra_heap(), task);
    task->core.entry = body;
    task->core.closure = task;
    astra_task_start(&task->core);
    astra_task_cancel(&task->core);
    if (!task->core.canceled) {
        printf("cancel was not recorded while the task was running\n");
        return 2;
    }
    if (!astra_task_join(&task->core)) { astra_task_raise(&task->core); }
    printf("after\n");
    astra_task_heap_free(&task->core);
    astra_runtime_shutdown();
    return 0;
}
"#;

#[test]
fn a_cancelled_task_without_a_value_raises_a_task_error_at_join() {
    let Some((ok, stdout, stderr)) = build_and_run(CANCELLED_TASK_BECOMES_TASK_ERROR, false) else {
        return;
    };
    assert!(
        !ok,
        "a cancelled task without a value must fail the joining task"
    );
    assert_eq!(stdout, "", "the join must raise before the next statement");
    assert!(
        stderr.contains("panic: TaskError: task was cancelled"),
        "expected a cancellation TaskError, got:\n{stderr}"
    );
}

const VALUE_WINS_OVER_CANCELLATION: &str = r#"
#include <stdatomic.h>

typedef struct { NS_Task_Core core; int value; } NS_Task_int;

static atomic_int started = 0;
static atomic_int release = 0;

static void body(void* arg) {
    NS_Task_int* task = (NS_Task_int*)arg;
    atomic_store(&started, 1);
    while (!atomic_load(&release)) { }
    task->value = 7;
}

int main(void) {
    NS_Task_int* task = (NS_Task_int*)astra_gc_alloc(&NS_SHAPE_LEAF, sizeof(NS_Task_int));
    astra_gc_root_in(astra_heap(), task);
    task->core.entry = body;
    task->core.closure = task;
    astra_task_start(&task->core);
    while (!atomic_load(&started)) { }
    astra_task_cancel(&task->core);
    if (!task->core.canceled) {
        printf("cancel was not recorded while the task was running\n");
        return 2;
    }
    atomic_store(&release, 1);
    if (!astra_task_join(&task->core)) { astra_task_raise(&task->core); }
    printf("%d\n", task->value);
    astra_task_heap_free(&task->core);
    astra_runtime_shutdown();
    return 0;
}
"#;

#[test]
fn a_value_that_lost_the_cancellation_race_is_still_joined() {
    let Some((ok, stdout, stderr)) = build_and_run(VALUE_WINS_OVER_CANCELLATION, false) else {
        return;
    };
    assert!(ok, "execution failed:\nstderr:\n{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["7"]);
}
