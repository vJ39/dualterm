use std::io::{self, Read, Write};
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::pty::{PtyCommand, PtyEngine};

pub const WS_PATH: &str = "/ws";

const READ_CHUNK: usize = 8192;
const QUEUE_DEPTH: usize = 32;

/// 接続ごとにPTYを起こすためのファクトリ。共有セッション方式へ差し替えられるよう関数で持つ。
pub type PtyFactory = Arc<dyn Fn() -> io::Result<PtyEngine> + Send + Sync>;

pub fn factory(command: PtyCommand) -> PtyFactory {
    Arc::new(move || PtyEngine::spawn(&command))
}

pub fn router(factory: PtyFactory) -> Router {
    Router::new()
        .route(WS_PATH, get(upgrade))
        .with_state(factory)
}

pub async fn serve(listener: TcpListener, factory: PtyFactory) -> io::Result<()> {
    axum::serve(listener, router(factory)).await
}

// 認証はCloudflare Access側で担保する前提なので、ここでは検査しない。
async fn upgrade(ws: WebSocketUpgrade, State(factory): State<PtyFactory>) -> Response {
    ws.on_upgrade(move |socket| async move {
        if let Ok(engine) = factory() {
            bridge(socket, engine).await;
        }
    })
}

pub async fn bridge(socket: WebSocket, mut engine: PtyEngine) {
    let (Ok(mut reader), Ok(mut writer)) = (engine.take_reader(), engine.take_writer()) else {
        return;
    };

    let (from_pty_tx, mut from_pty_rx) = mpsc::channel::<Vec<u8>>(QUEUE_DEPTH);
    let (to_pty_tx, mut to_pty_rx) = mpsc::channel::<Vec<u8>>(QUEUE_DEPTH);

    // PTYのread/writeはブロッキングなので、asyncランタイムを塞がないよう専用スレッドへ逃がす。
    std::thread::spawn(move || {
        let mut buf = [0u8; READ_CHUNK];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if from_pty_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    std::thread::spawn(move || {
        while let Some(chunk) = to_pty_rx.blocking_recv() {
            if writer.write_all(&chunk).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });

    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            outbound = from_pty_rx.recv() => match outbound {
                Some(chunk) => {
                    if sink.send(Message::Binary(chunk.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            inbound = stream.next() => match inbound {
                Some(Ok(Message::Binary(bytes))) => {
                    if to_pty_tx.send(bytes.into()).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Message::Text(text))) => {
                    if to_pty_tx.send(text.as_bytes().to_vec()).await.is_err() {
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            },
        }
    }

    let _ = sink.send(Message::Close(None)).await;
    drop(to_pty_tx);
    let _ = engine.kill();
}
