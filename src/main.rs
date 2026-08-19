use std::io;

use dualterm::pty::PtyCommand;
use dualterm::ws;

const DEFAULT_ADDR: &str = "127.0.0.1:7681";

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = std::env::var("DUALTERM_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "dualterm listening on ws://{}{}",
        listener.local_addr()?,
        ws::WS_PATH
    );

    ws::serve(listener, ws::factory(PtyCommand::default_shell())).await
}
