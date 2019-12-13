#![warn(rust_2018_idioms)]
use {
    std::env,
    tokio::net::TcpListener,
    tokio_util::codec::{Framed, BytesCodec, Decoder},
    futures::{SinkExt, StreamExt},
    futures_util::stream::SplitStream,
    bytes::{BytesMut, Bytes},
    async_std::fs::File,
    async_std::prelude::*,
};

/// Possible requests our clients can send us
enum Request {
    Upload { filename: String },
    Get { filename: String },
    None,
}

impl Request {
    fn parse(input: &str) -> Result<Request, String> {
        let mut parts = input.splitn(2, " ");
        match parts.next() {
            Some("UPLOAD") => {
                let filename = match parts.next() {
                    Some(key) => key,
                    None => return Err(format!("UPLOAD must be followed by a filename")),
                };
                Ok(Request::Upload {
                    filename: filename.to_string(),
                })
            }
            Some("GET") => {
                let filename = match parts.next() {
                    Some(key) => key,
                    None => return Err(format!("GET must be followed by a filename")),
                };
                Ok(Request::Get {
                    filename: filename.to_string(),
                })
            }
            Some(cmd) => Err(format!("unknown command: {}", cmd)),
            None => Err(format!("empty input")),
        }
    }
}

/// Responses to the `Request` commands above
enum Response {
    Get {
        value: String,
    },
    Error {
        msg: String,
    },
}

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
                    let framed = BytesCodec::new().framed(socket);

                    handle_request(framed).await;

                    // The connection will be closed at this point as `framed.next()` has returned `None`.
                });
            },
            Err(e) => println!("error accepting socket; error = {:?}", e),
        }
    }
}

// handle incomming request
async fn handle_request(framed: Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>) {
    let (request_details, framed) = get_request_details(framed).await;
    let request_line = std::str::from_utf8(&request_details).unwrap();

    // split framed stream into read/write streams
    let (mut ws, rs) = framed.split();

    let mut request = Request::None;

    match Request::parse(&request_line) {
        Ok(req) => { request = req; },
        Err(e) => { 
            println!("error parsing request; error = {:?}", e);
            // send error back to the client
            ws.send(Bytes::from(e)).await.unwrap();
        },
    };

    match request {
        Request::Upload {filename} => {
            upload_file(&filename, rs).await;
        },
        Request::Get {filename: _} => {unimplemented!()},
        Request::None => {unimplemented!()}
    }
}

// read filename from stream of bytes
async fn get_request_details(mut framed: Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>) -> (BytesMut, Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>) {
    let bytes = if let Some(result) = framed.next().await {
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
    (bytes, framed)
}

async fn upload_file(filename: &str, mut rs: SplitStream<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>>) {
    let filepath = format!("./tmp/{}", filename);
    let mut f = File::create(filepath).await.unwrap();

    // We loop while there are messages coming from the Stream `framed`.
    // The stream will return None once the client disconnects.
    while let Some(result) = rs.next().await {
        match result {
            Ok(bytes) => {
                f = f.write_all(&bytes).await.map(|_| f).unwrap();
            },
            Err(e) => println!("error on decoding from socket; error = {:?}", e),
        }
    }
}