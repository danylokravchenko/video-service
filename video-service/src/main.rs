#![warn(rust_2018_idioms)]
use tokio::net::TcpListener;
use tokio_util::codec::{Framed, BytesCodec, Decoder};
use futures::{SinkExt, StreamExt};
use std::env;
use std::io::Write;
use bytes::BytesMut;

#[tokio::main]
async fn main() {
    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8091 for connections.
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8091".to_string());
    // Next up we create a TCP listener which will listen for incoming
    // connections. This TCP listener is bound to the address we determined
    // above and must be associated with an event loop.
    let mut listener = TcpListener::bind(&addr).await.unwrap();
    println!("Listening on: {}", addr);

    // create directory where to store videos
    std::fs::create_dir_all("./tmp").unwrap();

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                // We'll `spawn` this client to ensure it
                // runs concurrently with all other clients. The `move` keyword is used
                // here to move ownership into the async closure.
                tokio::spawn(async move {
                    // We're parsing each socket with the `BytesCodec`
                    let mut framed = BytesCodec::new().framed(socket);

                    // get filename of incomming file
                    let filename = if let Some(result) = framed.next().await {
                        match result {
                            Ok(bytes) => {
                                bytes
                            },
                            Err(e) => {
                                println!("error on decoding from socket; error = {:?}", e);
                                BytesMut::new()
                            },
                        }
                    } else {
                        BytesMut::new()
                    };
                    
                    let filename = std::str::from_utf8(&filename).unwrap();

                    let filepath = format!("./tmp/{}", filename);
                    // File::create is blocking operation, use threadpool
                    let mut f = tokio::spawn(async move {
                        std::fs::File::create(filepath)
                    }).await.unwrap().unwrap();

                    // We loop while there are messages coming from the Stream `framed`.
                    // The stream will return None once the client disconnects.
                    while let Some(result) = framed.next().await {
                        match result {
                            Ok(bytes) => {
                                // filesystem operations are blocking, we have to use threadpool
                                f = tokio::spawn(async move {
                                    f.write_all(&bytes).map(|_| f)
                                }).await.unwrap().unwrap();
                            },
                            Err(e) => println!("error on decoding from socket; error = {:?}", e),
                        }
                    }
                    // The connection will be closed at this point as `lines.next()` has returned `None`.
                });
            },
            Err(e) => println!("error accepting socket; error = {:?}", e),
        }
    }
}