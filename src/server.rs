use chrono::Local;
use crossbeam_channel::{Receiver, select};
use lazy_static::lazy_static;
use prometheus::{Encoder, TextEncoder};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

lazy_static! {
    static ref encoder: TextEncoder = TextEncoder::new();
    static ref headers: String = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\n\r\n", encoder.format_type());
}

fn handle_connection(mut stream: TcpStream) {
    let remote = stream.peer_addr().unwrap();
    let mut reader = BufReader::new(&stream);
    let mut request = String::with_capacity(128);
    reader.read_line(&mut request).unwrap_or_default();

    stream.write_all(headers.as_bytes()).unwrap_or_default();
    let metrics = prometheus::gather();
    encoder.encode(&metrics, &mut stream).unwrap_or_default();

    stream.shutdown(std::net::Shutdown::Both).unwrap_or_default();
    println!("{} {} {}", Local::now().to_rfc3339(), remote.ip(), request.trim_end());
}

pub fn serve(port: u16, shutdown_rx: Receiver<()>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let listener = TcpListener::bind(("0.0.0.0", port)).expect("unable to start HTTP server");
        println!("Listening to connections on port {}", port);

        let (conn_tx, conn_rx) = crossbeam_channel::unbounded::<TcpStream>();
        let accept = listener.try_clone().expect("failed to clone listener");
        thread::spawn(move || {
            for stream in accept.incoming() {
                match stream {
                    Ok(stream) => {
                        if conn_tx.send(stream).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        loop {
            select! {
                recv(shutdown_rx) -> _ => {
                    println!("server thread shutting down");
                    drop(listener);  // closes listener socket which stops connection-accepting thread
                    return;
                },
                recv(conn_rx) -> conn => {
                    if let Ok(stream) = conn {
                        handle_connection(stream);
                    } else {
                        return;
                    }
                }
            }
        }
    })
}
