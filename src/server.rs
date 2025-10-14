use chrono::Local;
use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;
use std::sync::LazyLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

static HEADERS: LazyLock<String> = LazyLock::new(|| {
    [
        "HTTP/1.1 200 OK",
        &format!("Content-Type: {}", TextEncoder::new().format_type()),
        "\r\n",
    ]
    .join("\r\n")
});

async fn handle_connection(mut stream: TcpStream, remote: SocketAddr) {
    let (stream_read, mut stream_write) = stream.split();
    let mut reader = BufReader::new(stream_read);
    let mut request = String::with_capacity(128);
    reader.read_line(&mut request).await.unwrap_or_default();

    let mut buffer = Vec::with_capacity(8 * 1024);
    let metrics = prometheus::gather();
    TextEncoder::new().encode(&metrics, &mut buffer).unwrap_or_default();

    stream_write.write_all(HEADERS.as_bytes()).await.unwrap_or_default();
    stream_write.write_all(&buffer).await.unwrap_or_default();

    stream.shutdown().await.unwrap_or_default();
    println!("{} {} {}", Local::now().to_rfc3339(), remote.ip(), request.trim_end());
}

pub async fn serve(port: u16) {
    let listener = TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("unable to start HTTP server");
    println!("Listening to connections on port {}", port);

    loop {
        let (stream, remote) = listener.accept().await.expect("failed to accept connection");
        tokio::spawn(handle_connection(stream, remote));
    }
}
