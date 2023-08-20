use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;

fn check_disconnection(num_bytes: usize) -> bool {
    if num_bytes == 0 {
        println!("Client disconnected unexpectedly");
        return true;
    }
    return false;
}

fn handle_connection(mut connection: TcpStream) -> io::Result<()> {
    let mut read_bytes = 0;
    let mut request = [0u8; 1024];
    loop {
        let num_bytes = connection.read(&mut request[read_bytes..])?;
        if check_disconnection(num_bytes) {
            return Ok(());
        }
        read_bytes += num_bytes;
        if request.get(read_bytes - 4..read_bytes) == Some(b"\r\n\r\n") {
            break;
        }
    }
    let request = String::from_utf8_lossy(&request[..read_bytes]);
    println!("{request}");

    let response = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Length: 12\n",
        "Connection: close\r\n\r\n",
        "Hello world!"
    );
    let mut written_bytes = 0;
    loop {
        let num_bytes = connection.write(response[written_bytes..].as_bytes())?;
        if check_disconnection(num_bytes) {
            return Ok(());
        }
        written_bytes += num_bytes;
        if written_bytes == response.len() {
            break;
        }
    }

    connection.flush().unwrap_or_else(|err| {
        eprintln!("{err}");
    });

    Ok(())
}

fn main() {
    let listener = TcpListener::bind("localhost:3000").unwrap();
    loop {
        let (connection, _) = listener.accept().unwrap();
        std::thread::spawn(move || {
            if let Err(e) = handle_connection(connection) {
                println!("failed to handle connection: {e}");
            }
        });
    }
}
