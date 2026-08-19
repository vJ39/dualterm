use std::time::Duration;

use dualterm::pty::PtyCommand;
use dualterm::ws;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const TIMEOUT: Duration = Duration::from_secs(5);

type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start(command: PtyCommand) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let factory = ws::factory(command);
    tokio::spawn(async move {
        let _ = ws::serve(listener, factory).await;
    });
    format!("ws://{addr}{}", ws::WS_PATH)
}

async fn connect(url: &str) -> Client {
    let (client, _) = connect_async(url).await.expect("websocket handshake");
    client
}

async fn recv_until(client: &mut Client, needle: &str) -> String {
    let collected = tokio::time::timeout(TIMEOUT, async {
        let mut acc: Vec<u8> = Vec::new();
        while let Some(msg) = client.next().await {
            match msg.expect("websocket frame") {
                Message::Binary(bytes) => acc.extend_from_slice(&bytes),
                Message::Text(text) => acc.extend_from_slice(text.as_bytes()),
                Message::Close(_) => break,
                _ => {}
            }
            if String::from_utf8_lossy(&acc).contains(needle) {
                break;
            }
        }
        String::from_utf8_lossy(&acc).into_owned()
    })
    .await;

    match collected {
        Ok(seen) => seen,
        Err(_) => panic!("timed out waiting for {needle:?}"),
    }
}

#[tokio::test]
async fn bridges_client_bytes_into_the_pty_and_back() {
    let url = start(PtyCommand::new("cat")).await;
    let mut client = connect(&url).await;

    client
        .send(Message::binary(b"hello-ws\n".to_vec()))
        .await
        .expect("send");

    let seen = recv_until(&mut client, "hello-ws").await;
    assert!(seen.contains("hello-ws"), "unexpected output: {seen:?}");
}

#[tokio::test]
async fn text_frames_are_forwarded_as_raw_bytes() {
    let url = start(PtyCommand::new("cat")).await;
    let mut client = connect(&url).await;

    client
        .send(Message::text("text-marker\n"))
        .await
        .expect("send");

    let seen = recv_until(&mut client, "text-marker").await;
    assert!(seen.contains("text-marker"), "unexpected output: {seen:?}");
}

#[tokio::test]
async fn forwards_pty_output_without_client_input() {
    let url = start(PtyCommand::new("echo").arg("dualterm-ws-ok")).await;
    let mut client = connect(&url).await;

    let seen = recv_until(&mut client, "dualterm-ws-ok").await;
    assert!(
        seen.contains("dualterm-ws-ok"),
        "unexpected output: {seen:?}"
    );
}

#[tokio::test]
async fn each_connection_gets_its_own_pty() {
    let url = start(PtyCommand::new("cat")).await;
    let mut first = connect(&url).await;
    let mut second = connect(&url).await;

    first
        .send(Message::binary(b"first-marker\n".to_vec()))
        .await
        .expect("send");
    second
        .send(Message::binary(b"second-marker\n".to_vec()))
        .await
        .expect("send");

    let first_seen = recv_until(&mut first, "first-marker").await;
    let second_seen = recv_until(&mut second, "second-marker").await;

    assert!(
        !first_seen.contains("second-marker"),
        "connections shared a pty: {first_seen:?}"
    );
    assert!(
        !second_seen.contains("first-marker"),
        "connections shared a pty: {second_seen:?}"
    );
}
