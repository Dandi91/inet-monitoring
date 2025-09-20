use chrono::Local;
use prometheus::Encoder;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

pub fn serve(port: u16) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let encoder = prometheus::TextEncoder::new();
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: {}\r\n\r\n", encoder.format_type());

        let listener = TcpListener::bind(("0.0.0.0", port)).expect("unable to start HTTP server");
        println!("Listening to connections on port {}", port);

        loop {
            match listener.accept() {
                Ok((mut stream, remote)) => {
                    let mut reader = BufReader::new(&stream);
                    let mut request = String::with_capacity(128);
                    reader.read_line(&mut request).unwrap_or_default();

                    stream.write_all(headers.as_bytes()).unwrap_or_default();
                    let metrics = prometheus::gather();
                    encoder.encode(&metrics, &mut stream).unwrap_or_default();

                    stream.shutdown(std::net::Shutdown::Both).unwrap_or_default();
                    println!("{} {} {}", Local::now().to_rfc3339(), remote.ip(), request.trim_end());
                }
                Err(err) => eprintln!("cannot accept connection: {}", err),
            }
        }
    })
}
