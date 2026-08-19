mod support;

use std::time::Duration;

use dualterm::pty::{PtyCommand, PtyEngine, PtySize};

const TIMEOUT: Duration = Duration::from_secs(5);

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[test]
fn writes_reach_the_pty_and_come_back() {
    let mut engine = PtyEngine::spawn(&PtyCommand::new("cat")).expect("spawn cat");
    let out = support::collect(engine.take_reader().expect("reader"));

    engine.write(b"hello\n").expect("write to pty");

    let seen = support::wait_for(&out, "hello\r\n", TIMEOUT);
    assert!(seen.contains("hello\r\n"), "unexpected output: {seen:?}");

    engine.kill().expect("kill");
}

#[test]
fn child_process_receives_the_bytes_not_only_terminal_echo() {
    let mut engine =
        PtyEngine::spawn(&PtyCommand::new("tr").args(["a-z", "A-Z"])).expect("spawn tr");
    let out = support::collect(engine.take_reader().expect("reader"));

    engine.write(b"hello\n").expect("write to pty");

    let seen = support::wait_for(&out, "HELLO", TIMEOUT);
    assert!(seen.contains("HELLO"), "unexpected output: {seen:?}");

    engine.kill().expect("kill");
}

#[test]
fn reads_child_stdout_and_waits_for_exit() {
    let mut engine =
        PtyEngine::spawn(&PtyCommand::new("echo").arg("dualterm-pty-ok")).expect("spawn echo");
    let out = support::collect(engine.take_reader().expect("reader"));

    support::wait_for(&out, "dualterm-pty-ok", TIMEOUT);

    let status = engine.wait().expect("wait");
    assert!(status.success(), "unexpected exit status: {status:?}");
}

#[test]
fn injected_command_is_used_instead_of_the_login_shell() {
    let mut engine = PtyEngine::spawn(
        &PtyCommand::new("sh")
            .args(["-c", "printf '%s\\n' \"$DUALTERM_TEST_MARKER\""])
            .env("DUALTERM_TEST_MARKER", "injected-42"),
    )
    .expect("spawn sh");
    let out = support::collect(engine.take_reader().expect("reader"));

    support::wait_for(&out, "injected-42", TIMEOUT);
    let _ = engine.wait();
}

#[test]
fn resize_updates_the_pty_size() {
    let mut engine =
        PtyEngine::spawn(&PtyCommand::new("cat").size(size(24, 80))).expect("spawn cat");

    let before = engine.size().expect("size");
    assert_eq!((before.rows, before.cols), (24, 80));

    engine.resize(size(40, 100)).expect("resize");

    let after = engine.size().expect("size");
    assert_eq!((after.rows, after.cols), (40, 100));

    engine.kill().expect("kill");
}

#[test]
fn child_process_observes_the_resized_size() {
    let mut engine = PtyEngine::spawn(
        &PtyCommand::new("sh")
            .args(["-c", "read line; stty size"])
            .size(size(24, 80)),
    )
    .expect("spawn sh");
    let out = support::collect(engine.take_reader().expect("reader"));

    engine.resize(size(40, 100)).expect("resize");
    engine.write(b"\n").expect("write to pty");

    let seen = support::wait_for(&out, "40 100", TIMEOUT);
    assert!(seen.contains("40 100"), "unexpected output: {seen:?}");

    let _ = engine.wait();
}

#[test]
fn kill_terminates_the_child() {
    let mut engine = PtyEngine::spawn(&PtyCommand::new("cat")).expect("spawn cat");

    engine.kill().expect("kill");
    let status = engine.wait().expect("wait");

    assert!(
        !status.success(),
        "killed child reported success: {status:?}"
    );
}

#[test]
fn spawn_fails_for_a_missing_program() {
    let result = PtyEngine::spawn(&PtyCommand::new("dualterm-no-such-program-xyz"));
    assert!(result.is_err(), "expected spawn to fail");
}
