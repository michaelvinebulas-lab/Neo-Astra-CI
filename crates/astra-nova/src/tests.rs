use std::cell::RefCell;
use std::rc::Rc;

use crate::{
    BufferHost, Host, IntegerPolicy, NovaError, NovaOptions, Value, build_interpreter, values_equal,
};

struct Output {
    result: Result<Value, NovaError>,
    stdout: String,
    stderr: String,
}

fn run(source: &str) -> Output {
    run_with(source, NovaOptions::default())
}

fn run_with(source: &str, options: NovaOptions) -> Output {
    let stdout = Rc::new(RefCell::new(String::new()));
    let stderr = Rc::new(RefCell::new(String::new()));
    let host = CaptureHost {
        stdout: stdout.clone(),
        stderr: stderr.clone(),
    };
    let result = match build_interpreter(source, options, Box::new(host)) {
        Ok(mut interpreter) => interpreter.run(),
        Err(error) => Err(error),
    };
    let stdout = stdout.borrow().clone();
    let stderr = stderr.borrow().clone();
    Output {
        result,
        stdout,
        stderr,
    }
}

#[derive(Clone)]
struct CaptureHost {
    stdout: Rc<RefCell<String>>,
    stderr: Rc<RefCell<String>>,
}

impl Host for CaptureHost {
    fn stdout(&mut self, text: &str) {
        self.stdout.borrow_mut().push_str(text);
    }

    fn stderr(&mut self, text: &str) {
        self.stderr.borrow_mut().push_str(text);
    }
}

fn assert_ok(output: &Output) {
    if let Err(error) = &output.result {
        panic!("expected success, got {error:?}");
    }
}

#[test]
fn runs_hello_world() {
    let output = run(r#"
module app.main

fn main() {
    print("hello, Neo Astra")
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "hello, Neo Astra");
    assert_eq!(output.result.unwrap(), Value::Unit);
}

#[test]
fn calls_functions_and_recurses() {
    let output = run(r#"
fn factorial(n: i32) -> i32 {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}

fn main() {
    println(factorial(5))
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "120\n");
}

#[test]
fn constructs_structs_and_matches_enums() {
    let output = run(r#"
struct User {
    id: u64
    name: str
}

enum Status {
    Pending
    Active(reason: str)
}

fn describe(status: Status) -> str {
    return match status {
        Status.Pending => "pending"
        Status.Active(reason) => reason
    }
}

fn main() {
    let mut user = User { id: 1, name: "Astra" }
    user.name = "Neo"
    println(user.name)
    println(describe(Status.Pending))
    println(describe(Status.Active("ready")))
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "Neo\npending\nready\n");
}

#[test]
fn supports_arrays_maps_and_tuples() {
    let output = run(r#"
fn main() {
    let mut values = [1, 2, 3]
    values.push(4)
    let mut map = {"a": 1}
    map["b"] = 2
    let mut pair = (1, "one")
    pair.0 = 7
    println(values.len())
    println(values[1])
    println(map.get("b").unwrap())
    println(pair.0)
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "4\n2\n2\n7\n");
}

#[test]
fn preserves_map_order_and_unicode_string_semantics() {
    let output = run(r#"
fn main() {
    let mut map = {"b": 2, "a": 1}
    map.insert("b", 3)
    println(map)
    for key in map {
        print(key)
    }
    println("")
    println("aé😀".len())
    let mut scalars = 0
    for character in "aé😀" {
        scalars = scalars + 1
    }
    println(scalars)
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "{b: 3, a: 1}\nba\n7\n3\n");
}

#[test]
fn array_bounds_fail_with_a_panic() {
    let output = run(r#"
fn main() {
    let values = [1]
    println(values[2])
}
"#);
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected a bounds panic");
    };
    assert!(panic.message.contains("out of bounds"));
    assert_eq!(
        panic.stack.first().map(|frame| frame.function.as_str()),
        Some("main")
    );
}

#[test]
fn handles_options_without_implicit_unwrapping() {
    let output = run(r#"
fn find(flag: bool) -> Option<i32> {
    if flag {
        return Some(42)
    }
    return None
}

fn main() {
    match find(true) {
        Some(value) => println(value)
        None => println("missing")
    }
    match find(false) {
        Some(value) => println(value)
        None => println("missing")
    }
    println(find(false).unwrap_or(0))
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "42\nmissing\n0\n");
}

#[test]
fn propagates_and_matches_failures() {
    let output = run(r#"
struct InvalidAge(str)

fn parse_age(input: str) -> i32 ! InvalidAge | ParseIntError {
    if input.is_empty() {
        return !InvalidAge("Age cannot be empty")
    }
    return input.to_int()!
}

fn main() {
    match parse_age("42") {
        Ok(age) => println(age)
        Err(error) => println("bad")
    }
    match parse_age("not a number") {
        Ok(age) => println(age)
        Err(error) => println("bad")
    }
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "42\nbad\n");
}

#[test]
fn propagates_results_with_bang() {
    let output = run(r#"
fn parse(input: str) -> i32 ! ParseIntError {
    return input.to_int()!
}

fn doubled(input: str) -> i32 ! ParseIntError {
    return parse(input)! * 2
}

fn main() {
    match doubled("21") {
        Ok(value) => println(value)
        Err(error) => println(-1)
    }
    match doubled("bad") {
        Ok(value) => println(value)
        Err(error) => println(-1)
    }
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "42\n-1\n");
}

#[test]
fn runs_loops_with_break_and_continue() {
    let output = run(r#"
fn main() {
    let mut total = 0
    for value in [1, 2, 3, 4, 5] {
        if value == 2 {
            continue
        }
        if value == 5 {
            break
        }
        total = total + value
    }
    println(total)
    let mut index = 0
    while index < 3 {
        index = index + 1
    }
    println(index)
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "8\n3\n");
}

#[test]
fn wraps_by_default_and_reports_checked_overflow() {
    let output = run(r#"
fn main() {
    println(255u8 +% 1)
    match 255u8.checked_add(1) {
        Ok(value) => println(value)
        Err(error) => println("overflow")
    }
    println(255u8.wrapping_add(1))
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "0\noverflow\n0\n");
}

#[test]
fn runs_deferred_statements_in_reverse_order() {
    let output = run(r#"
fn main() {
    defer println("first")
    defer println("second")
    println("body")
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "body\nsecond\nfirst\n");
}

#[test]
fn reports_panic_with_a_stack() {
    let output = run(r#"
fn inner() {
    panic("boom")
}

fn main() {
    inner()
}
"#);
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected a panic");
    };
    assert_eq!(panic.message, "boom");
    assert_eq!(panic.stack.len(), 2);
    assert_eq!(panic.stack[0].function, "inner");
    assert_eq!(panic.stack[1].function, "main");
}

#[test]
fn aborts_without_running_defers() {
    let output = run(r#"
fn main() {
    defer println("cleanup")
    println("before")
    abort("stop")
}
"#);
    assert_eq!(output.stdout, "before\n");
    let NovaError::Abort(abort) = output.result.unwrap_err() else {
        panic!("expected an abort");
    };
    assert_eq!(abort.message, "stop");
}

#[test]
fn runs_spawned_tasks_deterministically() {
    let output = run(r#"
fn main() {
    let task = spawn || {
        println("task")
    }
    task.join()
    println("after")
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "task\nafter\n");
}

#[test]
fn spawn_infers_the_task_result_type() {
    let output = run(r#"
fn main() {
    let task = spawn || {
        21
    }
    println(task.join())
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "21\n");
}

#[test]
fn imported_values_do_not_fall_back_to_builtins() {
    let output = run(r#"
import { print } from "./shadow.ns"

fn main() {
    print("shadow")
}
"#);
    let NovaError::Unsupported { message, .. } = output.result.unwrap_err() else {
        panic!("expected an unsupported imported value");
    };
    assert!(message.contains("print"));
}

#[test]
fn reports_a_missing_entry_point() {
    let output = run(r#"
fn helper() {
    let value = 1
}
"#);
    let NovaError::MissingEntry { name } = output.result.unwrap_err() else {
        panic!("expected a missing entry point");
    };
    assert_eq!(name, "main");
}

#[test]
fn a_spawned_task_panic_fails_its_scope() {
    let output = run(r#"
fn main() {
    let task = spawn || {
        panic("task panic")
    }
    println("after")
}
"#);
    assert_eq!(output.stdout, "after\n");
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected a task panic");
    };
    assert_eq!(panic.message, "task panic");
}

#[test]
fn evaluates_constants_lazily_once() {
    let output = run(r#"
const answer: i32 = 40 + 2

fn main() {
    println(answer)
    println(answer)
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "42\n42\n");
}

#[test]
fn returns_frontend_diagnostics_unchanged() {
    let output = run(r#"
fn main() {
    missing()
}
"#);
    let NovaError::Diagnostics(diagnostics) = output.result.unwrap_err() else {
        panic!("expected frontend diagnostics");
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NS4002")
    );
}

#[test]
fn rejects_manual_memory_in_system_mode() {
    let options = NovaOptions {
        mode: astra_core::mode::Mode::System,
        infer_mode: false,
        ..NovaOptions::default()
    };
    let output = run_with(
        r#"
fn main() {
    {@memory:manual;
        let value = 1
    }
}
"#,
        options,
    );
    let NovaError::ManualMemoryUnsupported { .. } = output.result.unwrap_err() else {
        panic!("expected manual-memory rejection");
    };
}

#[test]
fn uses_safe_checked_arithmetic_when_requested() {
    let options = NovaOptions {
        integer_policy: IntegerPolicy::SafeChecked,
        ..NovaOptions::default()
    };
    let output = run_with(
        r#"
fn main() {
    let mut value: u8 = 255
    value = value + 1
}
"#,
        options,
    );
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected an overflow panic");
    };
    assert!(panic.message.contains("overflow"));
}

#[test]
fn debug_output_uses_the_stderr_host() {
    let output = run(r#"
fn main() {
    dbg("hello")
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "\"hello\"\n");
}

#[test]
fn compares_structural_values() {
    assert!(values_equal(
        &Value::array(vec![Value::Integer(crate::Integer::new(
            1,
            astra_core::types::Type::I32
        ))]),
        &Value::array(vec![Value::Integer(crate::Integer::new(
            1,
            astra_core::types::Type::I32
        ))])
    ));
}

#[test]
fn buffer_host_captures_both_streams() {
    let mut host = BufferHost::new();
    host.stdout("out");
    host.stderr("err");
    assert_eq!(host.stdout, "out");
    assert_eq!(host.stderr, "err");
}

#[test]
fn runs_utf8_string_methods() {
    let output = run(r#"
pub fn main() -> i32 {
    let text = "héllo"
    println(text.len())
    println(text.char_count())
    match text.byte_at(1) {
        Some(byte) => println(byte),
        None => println("none"),
    }
    match text.char_at(1) {
        Some(character) => println(character),
        None => println("none"),
    }
    match text.slice(0, 3) {
        Ok(part) => println(part),
        Err(error) => println("slice failed"),
    }
    match text.slice(0, 2) {
        Ok(part) => println(part),
        Err(error) => println("slice failed"),
    }
    println(text.bytes().len())
    println(text.chars().len())
    return 0
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "6\n5\n195\né\nhé\nslice failed\n6\n5\n");
    assert_eq!(
        output.result.unwrap(),
        Value::Integer(crate::Integer::new(0, astra_core::types::Type::I32))
    );
}

#[test]
fn reports_str_slice_and_index_failures() {
    let output = run(r#"
pub fn main() -> i32 {
    let text = "héllo"
    match text.slice(0, 99) {
        Ok(part) => println("ok"),
        Err(error) => println("out of bounds"),
    }
    match text.slice(3, 1) {
        Ok(part) => println("ok"),
        Err(error) => println("reversed"),
    }
    match text.slice(1, 2) {
        Ok(part) => println("ok"),
        Err(error) => println("split scalar"),
    }
    match text.byte_at(99) {
        Some(byte) => println(byte),
        None => println("no byte"),
    }
    match text.char_at(99) {
        Some(character) => println(character),
        None => println("no char"),
    }
    return 0
}
"#);
    assert_ok(&output);
    assert_eq!(
        output.stdout,
        "out of bounds\nreversed\nsplit scalar\nno byte\nno char\n"
    );
}

#[test]
fn allocates_and_mutates_bytes() {
    let output = run(r#"
pub fn main() -> i32 {
    let mut buffer = bytes.new(4)
    buffer[0] = 0xFF
    buffer[1] = 7
    println(buffer.len())
    println(buffer[0])
    println(buffer[1])
    println(buffer.is_empty())
    let part = buffer.slice(1, 3)
    println(part.len())
    for value in buffer {
        println(value)
    }
    let empty = bytes.new(0)
    println(empty.is_empty())
    return 0
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "4\n255\n7\nfalse\n2\n255\n7\n0\n0\ntrue\n");
}

#[test]
fn rejects_out_of_bounds_byte_writes() {
    let output = run(r#"
pub fn main() -> i32 {
    let mut buffer = bytes.new(2)
    buffer[5] = 1
    return 0
}
"#);
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected a panic for an out-of-bounds byte write");
    };
    assert!(
        panic.message.contains("out of bounds"),
        "unexpected panic message: {}",
        panic.message
    );
}

#[test]
fn runs_channels_eagerly() {
    let output = run(r#"
fn produce(channel: Channel<str>) -> () ! ChannelError {
    channel.send("hello")!
    channel.send("world")!
}

pub fn main() -> i32 {
    let channel = Channel.create<str>()
    spawn || {
        match produce(channel) {
            Ok(value) => println("sent"),
            Err(error) => println("send failed"),
        }
    }
    match channel.receive() {
        Ok(message) => println(message),
        Err(error) => println("closed"),
    }
    match channel.receive() {
        Ok(message) => println(message),
        Err(error) => println("closed"),
    }
    channel.close()
    match channel.send("late") {
        Ok(value) => println("sent late"),
        Err(error) => println("closed send"),
    }
    return 0
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "sent\nhello\nworld\nclosed send\n");
}

#[test]
fn closed_channels_fail_send_and_receive() {
    let output = run(r#"
pub fn main() -> i32 {
    let channel = Channel.create<str>()
    channel.close()
    match channel.send("late") {
        Ok(value) => println("sent"),
        Err(error) => println("closed send"),
    }
    match channel.receive() {
        Ok(message) => println(message),
        Err(error) => println("closed receive"),
    }
    return 0
}
"#);
    assert_ok(&output);
    assert_eq!(output.stdout, "closed send\nclosed receive\n");
}

#[test]
fn sending_to_a_full_channel_deadlocks() {
    let output = run(r#"
fn fill(channel: Channel<i32>) -> () ! ChannelError {
    let mut index = 0
    while index < 17 {
        channel.send(index)!
        index = index + 1
    }
}

pub fn main() -> i32 {
    let channel = Channel.create<i32>()
    match fill(channel) {
        Ok(value) => println("filled"),
        Err(error) => println("failed"),
    }
    return 0
}
"#);
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected a deadlock panic when the channel fills");
    };
    assert!(
        panic
            .message
            .contains("channel deadlock: send on a full bounded channel"),
        "unexpected panic message: {}",
        panic.message
    );
}

#[test]
fn receiving_from_an_empty_channel_deadlocks() {
    let output = run(r#"
pub fn main() -> i32 {
    let channel = Channel.create<i32>()
    match channel.receive() {
        Ok(value) => println("received"),
        Err(error) => println("empty"),
    }
    return 0
}
"#);
    let NovaError::Panic(panic) = output.result.unwrap_err() else {
        panic!("expected a deadlock panic when the channel is empty");
    };
    assert!(
        panic
            .message
            .contains("channel deadlock: receive on an empty channel"),
        "unexpected panic message: {}",
        panic.message
    );
}
