#![warn(rust_2018_idioms)]

pub mod video_client{
        
    use {
        tokio::prelude::*,
        tokio::net::{TcpStream},
        tokio_util::{codec::{Framed, BytesCodec, Decoder}},
        futures_util::stream::{SplitStream, SplitSink},
        bytes::{Bytes, BytesMut},
        futures::{SinkExt, StreamExt},
        std::{error::Error, net::SocketAddr},
    };

    /// Client allows to communicate with remote video-service 
    pub struct VideoClient {
        addr: SocketAddr, 
    }

    impl VideoClient {
        /// create new client
        pub fn new(addr: SocketAddr) -> VideoClient {
            VideoClient{addr}
        }

        /// create new socket connection to remote video service
        pub async fn conn(& self) -> Result<(VideoConnection), Box<dyn Error>> {
            let stream = TcpStream::connect(&self.addr).await?;
            let framed = BytesCodec::new().framed(stream);
            // TODO: now it is possible to handle errors
            // split framed stream into write/read parts
            let (sink, stream) = framed.split();
            // 2^24 = 16777216
            Ok(VideoConnection{ stream, sink, buffer: BytesMut::with_capacity(16777216) })
        }
    }

    pub struct VideoConnection {
        stream: SplitStream<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>>,
        sink: SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>,
        // stream: TcpStream,
        buffer: BytesMut,
    }

    impl VideoConnection {
        /// start uploading and send a filename of video file to remote video service
        pub async fn start_uploading(&mut self, filename: &str) -> Result<(), Box<dyn Error>> {
            let cmd = [b"UPLOAD ", filename.as_bytes()].concat();
            self.sink.send(Bytes::from(cmd)).await?;
            // get response to our sending command
            self.get_response().await
        }

        /// send a chunk of data to remote video service using buffer
        pub async fn buffered_send(&mut self, bytes: BytesMut) -> Result<(), Box<dyn Error>> {
            self.buffer.extend_from_slice(&bytes);
            // 2^23 = 8388608
            if self.buffer.len() >= 8388608 {
                self.sink.send(Bytes::copy_from_slice(&self.buffer)).await?;
                self.buffer.clear();
            }
            Ok(())
        }

        /// flush the rest of bytes in buffer into stream
        pub async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
            self.sink.send(Bytes::copy_from_slice(&self.buffer)).await?;
            self.buffer.clear();
            Ok(())
        }

        /// start recieving a videofile
        pub async fn start_recieving(&mut self, filename: &str) -> Result<(), Box<dyn Error>> {
            let cmd = [b"GET ", filename.as_bytes()].concat();
            self.sink.send(Bytes::from(cmd)).await?;
            // get response to our sending command
            self.get_response().await
        }

        /// read incomming bytes from the stream
        pub async fn read_next(&mut self) -> Result<BytesMut, std::io::Error> {        
            self.stream.next().await.unwrap()
        }

        /// get response to our sended command
        /// possible response is OK and ERROR with message why its happend
        async fn get_response(&mut self) -> Result<(), Box<dyn Error>> {
            let response_details = self.get_response_details().await?;
            let response_line = std::str::from_utf8(&response_details)?;

            let mut response = Response::None;
            match Response::parse(&response_line) {
                Ok(req) => { response = req; },
                Err(e) => { 
                    println!("error parsing request; error = {:?}", e);
                },
            };

            match response {
                Response::Ok => {
                    Ok(())
                },
                Response::Error {msg} => {
                    Err(format!("{}", msg))?
                },
                Response::None => {unimplemented!()}
            }
        }

        /// helper to read response bytes from stream
        async fn get_response_details(&mut self) -> Result<BytesMut, Box<dyn Error>> {
            if let Some(result) = self.stream.next().await {
                match result {
                    Ok(bytes) => {
                        Ok(bytes)
                    },
                    Err(e) => {
                        Err(format!("error on decoding from socket; error = {:?}", e))?
                    },
                }
            } else {
                Err(format!("There is no response from video service"))?
            }
        }
       
    }


    /// Possible response video service could response with
    enum Response {
        Ok,
        Error { msg: String },
        None,
    }

    impl Response {
        fn parse(input: &str) -> Result<Response, String> {
            let mut parts = input.splitn(2, " ");
            match parts.next() {
                Some("OK") => {
                    Ok(Response::Ok)
                }
                Some("ERROR") => {
                    let msg = match parts.next() {
                        Some(key) => key,
                        None => return Err(format!("ERROR must be followed by a message")),
                    };
                    Ok(Response::Error {
                        msg: msg.to_string(),
                    })
                }
                Some(cmd) => Err(format!("unknown command: {}", cmd)),
                None => Err(format!("empty input")),
            }
        }
    }

}

mod codec {
    use bytes::{BufMut, BytesMut};
    use std::io;
    use tokio_util::codec::{Decoder, Encoder};

    /// A simple `Codec` implementation that just ships bytes around.
    ///
    /// This type is used for "framing" a TCP/UDP stream of bytes but it's really
    /// just a convenient method for us to work with streams/sinks for now.
    /// This'll just take any data read and interpret it as a "frame" and
    /// conversely just shove data into the output location without looking at
    /// it.
    pub struct Bytes;

    impl Decoder for Bytes {
        type Item = Vec<u8>;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Vec<u8>>> {
            if !buf.is_empty() {
                let len = buf.len();
                Ok(Some(buf.split_to(len).into_iter().collect()))
            } else {
                Ok(None)
            }
        }
    }

    impl Encoder for Bytes {
        type Item = Vec<u8>;
        type Error = io::Error;

        fn encode(&mut self, data: Vec<u8>, buf: &mut BytesMut) -> io::Result<()> {
            buf.put(&data[..]);
            Ok(())
        }
    }
}