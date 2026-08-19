#![allow(dead_code)]

use std::io::Read;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// PTYの読み取りはブロッキングなので、スレッドへ逃がしてタイムアウト付きで検証する。
pub fn collect(mut reader: Box<dyn Read + Send>) -> Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    rx
}

pub fn wait_for(rx: &Receiver<Vec<u8>>, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut acc: Vec<u8> = Vec::new();
    loop {
        let seen = String::from_utf8_lossy(&acc).into_owned();
        if seen.contains(needle) {
            return seen;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            panic!("timed out waiting for {needle:?}, got {seen:?}");
        }
        match rx.recv_timeout(left) {
            Ok(chunk) => acc.extend_from_slice(&chunk),
            Err(RecvTimeoutError::Timeout) => {
                panic!("timed out waiting for {needle:?}, got {seen:?}")
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!("pty output closed before {needle:?}, got {seen:?}")
            }
        }
    }
}
