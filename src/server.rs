use chrono::Local;
use lazy_static::lazy_static;
use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

lazy_static! {
    static ref encoder: TextEncoder = TextEncoder::new();
    static ref headers: String = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\n\r\n", encoder.format_type());
}

async fn handle_connection(mut stream: TcpStream, remote: SocketAddr) {
    let (stream_read, mut stream_write) = stream.split();
    let mut reader = BufReader::new(stream_read);
    let mut request = String::with_capacity(128);
    reader.read_line(&mut request).await.unwrap_or_default();

    let mut buffer = Vec::with_capacity(8 * 1024);
    let metrics = prometheus::gather();
    encoder.encode(&metrics, &mut buffer).unwrap_or_default();

    stream_write.write_all(headers.as_bytes()).await.unwrap_or_default();
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
