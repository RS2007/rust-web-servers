use std::io::Read;
use std::io::Write;
use std::net::TcpListener;

enum ConnectionState {
    Read {
        request: [u8; 1024],
        read: usize,
    },
    Write {
        response: &'static [u8],
        written: usize,
    },
    Flush,
}

fn main() {
    let mut connections = Vec::new();
    let listener = TcpListener::bind("localhost:3000").unwrap();
    listener.set_nonblocking(true).unwrap();

    loop {
        match listener.accept() {
            Ok((connection, _)) => {
                connection.set_nonblocking(true).unwrap();
                let state = ConnectionState::Read {
                    request: [0u8; 1024],
                    read: 0,
                };
                connections.push((connection, state));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {} // ref: https://doc.rust-lang.org/rust-by-example/flow_control/match/guard.html

            Err(e) => panic!("{e}"),
        };

        let mut completed = Vec::new();
        'next: for (i, (connection, state)) in connections.iter_mut().enumerate() {
            if let ConnectionState::Read { request, read } = state {
                loop {
                    match connection.read(&mut request[*read..]) {
                        Ok(0) => {
                            println!("client disconnected unexpectedly");
                            completed.push(i);
                            continue 'next;
                        }
                        Ok(n) => *read += n,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            continue 'next;
                        }
                        Err(e) => panic!("{e}"),
                    }
                    if request.get(*read - 4..*read) == Some(b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request[..*read]);
                println!("{request}");

                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Length: 12\n",
                    "Connection: close\r\n\r\n",
                    "Hello world!"
                );
                *state = ConnectionState::Write {
                    response: response.as_bytes(),
                    written: 0,
                }
            }
            if let ConnectionState::Write { response, written } = state {
                loop {
                    match connection.write(&response[*written..]) {
                        Ok(0) => {
                            println!("Client disconnected abruptly");
                            completed.push(i);
                            continue 'next;
                        }
                        Ok(n) => {
                            *written += n;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            continue 'next;
                        }
                        Err(e) => panic!("{e}"),
                    }
                    if *written == response.len() {
                        break;
                    }
                }
                *state = ConnectionState::Flush {};
            }
            if let ConnectionState::Flush = state {
                match connection.flush() {
                    Ok(_) => {
                        completed.push(i);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue 'next;
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        }

        for i in completed.into_iter().rev() {
            connections.remove(i);
        }
    }
}
