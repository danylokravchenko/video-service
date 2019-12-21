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

// custom types to simplify code
type FramedStream = Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type ReadStream = SplitStream<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>>;
type WriteStream = SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>;

/// Possible requests our clients can send us
enum Request {
    Upload { filename: String },
    Get { filename: String },
    None,
}

/// Possible response to our client
enum Command {
    Ok,
    Err { msg: String },
}

impl Request {
    /// parse request and handle errors
    fn parse(input: &str) -> std::result::Result<Request, String> {
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
async fn main() -> Result<()> {
    // Allow passing an address to listen on as the first argument of this
    // program, but otherwise we'll just set up our TCP listener on
    // 127.0.0.1:8091 for connections.
    let addr = env::args().nth(1).unwrap_or("127.0.0.1:8091".to_string());
    // Next up we create a TCP listener which will listen for incoming
    // connections. This TCP listener is bound to the address we determined
    // above and must be associated with an event loop.
    let mut listener = TcpListener::bind(&addr).await?;
    println!("Listening on: {}", addr);

    // create directory where to store temp videos before compressing
    std::fs::create_dir_all("./tmp")?;

    // create directory where to store compressed videos
    std::fs::create_dir_all("./dist")?;

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                // We'll `spawn` this client to ensure it
                // runs concurrently with all other clients. The `move` keyword is used
                // here to move ownership into the async closure.
                tokio::spawn(async move {
                    // We're parsing each socket with the `BytesCodec`
                    let framed = BytesCodec::new().framed(socket);

                    handle_request(framed).await.unwrap();

                    // The connection will be closed at this point as `framed.next()` has returned `None`.
                });
            },
            Err(e) => println!("error accepting socket; error = {:?}", e),
        }
    }
}

// handle incomming request
async fn handle_request(mut framed: FramedStream) -> Result<()> {
    let request_details = get_request_details(&mut framed).await.unwrap_or(BytesMut::new());
    let request_line = std::str::from_utf8(&request_details)?;

    // split framed stream into read/write streams
    let (mut ws, rs) = framed.split();

    let mut request = Request::None;
    match Request::parse(&request_line) {
        Ok(req) => { request = req; },
        Err(e) => { 
            println!("error parsing request; error = {:?}", e);
            send_cmd(&mut ws, Command::Err{msg: e}).await?;
        },
    };

    match request {
        Request::Upload {filename} => {
            upload_file(&filename, rs, ws).await?;
        },
        Request::Get {filename} => {
            send_file(&filename, ws).await?;
        },
        Request::None => {unimplemented!()}
    }

    Ok(())
} 

// read filename from stream of bytes
async fn get_request_details(framed: &mut FramedStream) -> Result<BytesMut> {
    if let Some(result) = framed.next().await {
        match result {
            Ok(bytes) => {
                return Ok(bytes);
            },
            Err(e) => {
                return Err(format!("error on decoding from socket; error = {:?}", e).into());
            },
        }
    }
    Err(format!("nothing comes from the stream").into())
}

// create a file from incomming bytes and handle errors
async fn upload_file(filename: &str, mut rs: ReadStream, mut ws: WriteStream) -> Result<()> {
    let filepath = format!("./tmp/{}", filename);

    if Path::new(&filepath).exists().await {
        let e = format!("file already exists");
        // send error back to the client
        send_cmd(&mut ws, Command::Err{msg: e}).await?;
        return Ok(());
    }

    send_cmd(&mut ws, Command::Ok).await?;

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

// send file to the client
async fn send_file(filename: &str, mut ws: WriteStream) -> Result<()> {
    let filepath = format!("./tmp/{}", filename);

    if !Path::new(&filepath).exists().await {
        let e = format!("file does not exist");
        // send error back to the client
        send_cmd(&mut ws, Command::Err{msg: e}).await?;
        return Ok(());
    }

    send_cmd(&mut ws, Command::Ok).await?;

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

// send response command to the client
async fn send_cmd(ws: &mut WriteStream, cmd: Command) -> Result<()>{
    let cmd = match cmd {
        Command::Err{msg} => {
            [b"ERROR ", msg.as_bytes()].concat()
        },
        Command::Ok => {
            [b"OK", " ".as_bytes()].concat()
        }
    };
    ws.send(Bytes::from(cmd)).await?;
    Ok(())
}