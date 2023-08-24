use epoll::{ControlOptions::*, Event, Events};
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::os::unix::io::AsRawFd;

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
    let listener = TcpListener::bind("localhost:3000").unwrap();
    listener.set_nonblocking(false).unwrap();
    let epoll = epoll::create(false).unwrap();
    let event = Event::new(Events::EPOLLIN, listener.as_raw_fd() as _);
    epoll::ctl(epoll, EPOLL_CTL_ADD, listener.as_raw_fd(), event).unwrap();
    let mut connections = HashMap::new();
    loop {
        let mut events = [Event::new(Events::empty(), 0); 1024];
        let timeout = -1; // block
        let num_events = epoll::wait(epoll, timeout, &mut events).unwrap();
        let mut completed = Vec::new();
        'next: for event in &events[..num_events] {
            let fd = event.data as i32;
            if fd == listener.as_raw_fd() {
                match listener.accept() {
                    Ok((connection, _)) => {
                        connection.set_nonblocking(false).unwrap();
                        let fd = connection.as_raw_fd();
                        let event = Event::new(Events::EPOLLIN | Events::EPOLLOUT, fd as _);
                        let _ = epoll::ctl(epoll, EPOLL_CTL_ADD, fd, event);

                        let state = ConnectionState::Read {
                            request: [0u8; 1024],
                            read: 0,
                        };
                        connections.insert(fd, (connection, state));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("{e}"),
                }
                continue 'next;
            }

            let (connection, state) = connections.get_mut(&fd).unwrap();

            if let ConnectionState::Read { request, read } = state {
                loop {
                    match connection.read(&mut request[*read..]) {
                        Ok(0) => {
                            completed.push(fd);
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
                println!("{}", request);
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
                            completed.push(fd);
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
                *state = ConnectionState::Flush;
            }
            if let ConnectionState::Flush = state {
                match connection.flush() {
                    Ok(_) => {
                        completed.push(fd);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        continue 'next;
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        }

        for fd in &completed {
            let (connection, _state) = connections.remove(&fd).unwrap();
            drop(connection);
        }
    }
}
