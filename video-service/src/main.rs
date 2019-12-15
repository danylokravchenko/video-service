#![warn(rust_2018_idioms)]

use {
    std::env,
    tokio::net::TcpListener,
    tokio_util::codec::{Framed, BytesCodec, Decoder},
    futures::{SinkExt, StreamExt},
    futures_util::stream::{SplitStream, SplitSink},
    bytes::{BytesMut, Bytes},
    async_std::{fs::File, path::Path},
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

    // create directory where to store temp videos before compressing
    std::fs::create_dir_all("./tmp").unwrap();

    // create directory where to store compressed videos
    std::fs::create_dir_all("./dist").unwrap();

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
            let err = [b"ERROR ", e.as_bytes()].concat();
            // send error back to the client
            ws.send(Bytes::from(err)).await.unwrap();
        },
    };

    match request {
        Request::Upload {filename} => {
            upload_file(&filename, rs, ws).await.unwrap();
        },
        Request::Get {filename} => {
            send_file(&filename, ws).await.unwrap();
        },
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

async fn upload_file(filename: &str, 
    mut rs: SplitStream<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>>,
    mut ws: SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>
) -> async_std::io::Result<()> {
    let filepath = format!("./tmp/{}", filename);

    if Path::new(&filepath).exists().await {
        let e = format!("file already exists");
        let err = [b"ERROR ", e.as_bytes()].concat();
        // send error back to the client
        ws = send_cmd(err, ws).await;
        return Ok(());
    }

    ws = send_cmd([b"OK", " ".as_bytes()].concat(), ws).await;

    let mut f = File::create(filepath).await?;

    // We loop while there are messages coming from the Stream `framed`.
    // The stream will return None once the client disconnects.
    while let Some(result) = rs.next().await {
        match result {
            Ok(bytes) => {
                f = f.write_all(&bytes).await.map(|_| f)?;
            },
            Err(e) => println!("error on decoding from socket; error = {:?}", e),
        }
    }

    // for now, I don't want to compress videos, because I don't see any benefits of using it
    // let source = format!("./tmp/{}", filename);
    // let destination = format!("./dist/{}", filename);

    // // compressing video is blocking operation, use threadpool
    // tokio::spawn(async move {
    //     compress_file_lz4(&source, &destination).unwrap();
    //     //compress_file_brotli(&source, &destination).unwrap();
    // }).await.unwrap();

    Ok(())
}

async fn send_file(filename: &str, 
    mut ws: SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>
) -> async_std::io::Result<()> {
    let filepath = format!("./tmp/{}", filename);

    if !Path::new(&filepath).exists().await {
        let e = format!("file does not exist");
        let err = [b"ERROR ", e.as_bytes()].concat();
        // send error back to the client
        ws = send_cmd(err, ws).await;
        return Ok(());
    }

    ws = send_cmd([b"OK", " ".as_bytes()].concat(), ws).await;

    let mut f = File::open(filepath).await?;

    const LEN: usize = 8388608; // 8 and something Mb
    let mut buf = vec![0u8; LEN];

    loop {  
        // Read a buffer from the file.
        let n = f.read(&mut buf).await?;

        // If this is the end of file, clean up and return.
        if n == 0 {
            return Ok(());
        }

        // Write the buffer into stream.
        ws.send(Bytes::copy_from_slice(&buf[..n])).await?;
    }

}

async fn send_cmd(cmd: Vec<u8>,
    mut ws: SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>
) -> SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>{
    ws.send(Bytes::from(cmd)).await.unwrap();
    ws
}