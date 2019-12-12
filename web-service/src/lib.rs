#![warn(rust_2018_idioms)]

pub mod video_client{
        
    use tokio::io;
    use tokio::prelude::*;
    use futures::{future, Sink, SinkExt, Stream, StreamExt};
    use std::{env, error::Error, net::SocketAddr};
    use tokio::net::TcpStream;
    use tokio_util::codec::{FramedRead, FramedWrite};
    use bytes::{Bytes, BytesMut};

    /// Client allows to communicate with remote video-service 
    pub struct VideoClient {
        stream: TcpStream,
        buffer: BytesMut,
    }

    impl VideoClient {
        /// create new client and connect to remote video service
        pub async fn new(addr: &SocketAddr) -> Result<(VideoClient), Box<dyn Error>> {
            let stream = TcpStream::connect(addr).await?;
            // 2^24 = 16777216
            Ok(VideoClient{ stream, buffer: BytesMut::with_capacity(16777216) })
        }

        /// send a chunk of data to remote bideo service
        pub async fn send(&mut self, bytes: Bytes) -> Result<(), Box<dyn Error>> {
            // TODO: try to use buffer and every time someone calls this method
            // check whenever buffer is almost full and flushes it in stream
            // else append bytes and return 
            self.buffer.extend_from_slice(&bytes);
            // 2^23 = 8388608
            if self.buffer.len() >= 8388608 {
                self.stream.write_all(&self.buffer).await?;
                self.buffer.clear();
            }
            Ok(())
        }

        /// flush the rest of bytes in buffer into stream
        pub async fn flush(&mut self) -> Result<(), Box<dyn Error>> {
            self.stream.write_all(&self.buffer).await?;
            self.buffer.clear();
            Ok(())
        }
    }


    // It is impossible to test async now :(
    // #[cfg(test)]
    // mod test_video_client {
    //     use super::VideoClient;
    //     use super::SocketAddr;
    //     use futures::executor::block_on;

    //     #[test]
    //     fn connection() {
    //         let remote_adr: SocketAddr = "127.0.0.1:8091".to_string()
    //             .parse()
    //             .expect("Remote adress structure is not valid");
    //         block_on(VideoClient::new(&remote_adr));
            
    //     }
    // }


        // pub async fn connect(
        //     addr: &SocketAddr,
        //     stdin: impl Stream<Item = Result<Vec<u8>, io::Error>> + Unpin,
        //     mut stdout: impl Sink<Vec<u8>, Error = io::Error> + Unpin,
        // ) -> Result<(), Box<dyn Error>> {
        //     let mut stream = TcpStream::connect(addr).await?;
        //     let (r, w) = stream.split();
        //     let sink = FramedWrite::new(w, codec::Bytes);
        //     let mut stream = FramedRead::new(r, codec::Bytes)
        //         .filter_map(|i| match i {
        //             Ok(i) => future::ready(Some(i)),
        //             Err(e) => {
        //                 println!("failed to read from socket; error={}", e);
        //                 future::ready(None)
        //             }
        //         })
        //         .map(Ok);

        //     match future::join(stdin.forward(sink), stdout.send_all(&mut stream)).await {
        //         (Err(e), _) | (_, Err(e)) => Err(e.into()),
        //         _ => Ok(()),
        //     }
        // }
    // }
}