#![warn(rust_2018_idioms)]
pub mod video_client{
        
    use {
        tokio::net::{TcpStream},
        tokio_util::{codec::{Framed, BytesCodec, Decoder}},
        futures_util::stream::{SplitStream, SplitSink},
        bytes::{Bytes, BytesMut},
        futures::{SinkExt, StreamExt},
        std::{net::SocketAddr},
    };

    /// Client allows to communicate with remote video-service 
    pub struct VideoClient {
        addr: SocketAddr, 
    }

    // custom types to simplify code
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
    type ReadStream = SplitStream<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>>;
    type WriteStream = SplitSink<Framed<tokio::net::TcpStream, tokio_util::codec::BytesCodec>, Bytes>;

    impl VideoClient {
        /// create new client
        pub fn new(addr: SocketAddr) -> VideoClient {
            VideoClient{addr}
        }

        /// create new socket connection to remote video service
        pub async fn conn(& self) -> Result<VideoConnection> {
            let stream = TcpStream::connect(&self.addr).await?;
            let framed = BytesCodec::new().framed(stream);
            // split framed stream into write/read parts
            let (sink, stream) = framed.split();
            // 2^24 = 16777216
            Ok(VideoConnection{ stream, sink, buffer: BytesMut::with_capacity(16777216) })
        }
    }

    pub struct VideoConnection {
        stream: ReadStream,
        sink: WriteStream,
        buffer: BytesMut,
    }

    impl VideoConnection {
        /// start uploading and send a filename of video file to remote video service
        pub async fn start_uploading(&mut self, filename: &str) -> Result<()> {
            let cmd = [b"UPLOAD ", filename.as_bytes()].concat();
            self.sink.send(Bytes::from(cmd)).await?;
            // get response to our sending command
            self.get_response().await
        }

        /// send a chunk of data to remote video service using buffer
        pub async fn buffered_send(&mut self, bytes: Bytes) -> Result<()> {
            self.buffer.extend_from_slice(&bytes);
            // 2^23 = 8388608
            if self.buffer.len() >= 8388608 {
                self.sink.send(Bytes::copy_from_slice(&self.buffer)).await?;
                self.buffer.clear();
            }
            Ok(())
        }

        /// flush the rest of bytes in buffer into stream
        pub async fn flush(&mut self) -> Result<()> {
            self.sink.send(Bytes::copy_from_slice(&self.buffer)).await?;
            self.buffer.clear();
            Ok(())
        }

        /// start recieving a videofile
        pub async fn start_recieving(&mut self, filename: &str) -> Result<()> {
            let cmd = [b"GET ", filename.as_bytes()].concat();
            self.sink.send(Bytes::from(cmd)).await?;
            // get response to our sending command
            self.get_response().await
        }

        /// read incomming bytes from the stream
        pub async fn read_next(&mut self) -> Option<std::result::Result<Bytes, std::io::Error>> {
            if let Some(bytes) = self.stream.next().await {
                // convert BytesMut to Bytes to satisfy HTTP Streaming trait
                Some(Ok(bytes.unwrap().freeze()))
            } else {
                None
            }
        }

        /// get response to our sended command
        /// possible response is OK and ERROR with message why its happend
        async fn get_response(&mut self) -> Result<()> {
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
        async fn get_response_details(&mut self) -> Result<BytesMut> {
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
        fn parse(input: &str) -> Result<Response> {
            let mut parts = input.splitn(2, " ");
            match parts.next() {
                Some("OK") => {
                    Ok(Response::Ok)
                }
                Some("ERROR") => {
                    let msg = match parts.next() {
                        Some(key) => key,
                        None => return Err(format!("ERROR must be followed by a message").into()),
                    };
                    Ok(Response::Error {
                        msg: msg.to_string(),
                    })
                }
                Some(cmd) => Err(format!("unknown command: {}", cmd).into()),
                None => Err(format!("empty input").into()),
            }
        }
    }

}

pub mod video_service {

    use {
        mysql_async::prelude::*,
        mysql_async::{Pool},
        chrono::{Utc, NaiveDateTime},
        serde::{Deserialize, Serialize},
    };

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    #[macro_export]
    macro_rules! list_fields {
        (struct $name:ident { $($fname:ident : $ftype:ty),* }) => {
            #[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize, Default)]
            pub struct $name {
                $(pub $fname : $ftype),*
            }
    
            impl $name {
                pub fn field_names() -> Vec<&'static str> {
                    vec![$(stringify!($fname)),*]
                }
            }
        }
    }
    
    // #[derive(Debug, PartialEq, Eq, Clone, Deserialize, Default)]
    // pub struct Video {
    //     pub id: i32,
    //     pub name: String,
    //     pub createdat: Option<String>,
    // }
    list_fields!{
        struct Video {
            id: i32,
            name: String,
            createdat: Option<String>
        }
    }
    
    pub struct VideoService {
        pool: Pool,
    }

    impl VideoService {
        pub fn new(pool: Pool) -> VideoService {
            VideoService{pool}
        }

        /// insert new video
        pub async fn insert_video(&self, filename: &str) -> Result<()> {
            let conn = self.pool.get_conn().await?;
            let now = Utc::now().format("%Y-%m-%d %H:%M:%S");
            let filename = filename.to_owned();
            let video = vec![Video{id: 0, name: filename, createdat: Some(now.to_string())}];
            let params = video.into_iter().map(|video| {
                params! {
                    "name" => video.name,
                    "createdat" => video.createdat.clone(),
                }
            });

            conn.batch_exec(r"INSERT INTO videos (name, createdat)
            VALUES (:name, :createdat)", params).await?;
            Ok(())
        }

        /// find video by id
        pub async fn find_video(&self, id: i32) -> Option<Video> {
            let conn = self.pool.get_conn().await.unwrap();
            let fields = Video::field_names().join(", ");

            let sql = format!(r"
                SELECT {} 
                FROM videos 
                WHERE id = :id
            ", fields);      
            let result = conn.prep_exec(sql, params!{"id" => id}).await.unwrap();
            
            // Collect result
            let (_ /* conn */, video) = result.map_and_drop(|row| {
                let (id, name, createdat) = mysql_async::from_row::<(i32, Vec<u8>, Option<NaiveDateTime>)>(row);
                Video {
                    id: id,
                    name: String::from_utf8_lossy(&name).into_owned(),
                    createdat: Some(createdat.unwrap().to_string()),
                }
            }).await.unwrap();

            if video.len() == 0 {
                None
            } else {
                Some(video[0].clone())
            }
        }

        /// list all videos from the database
        pub async fn list_videos(&self) -> Vec<Video> {
            let conn = self.pool.get_conn().await.unwrap();
            let fields = Video::field_names().join(", ");

            let sql = format!(r"
                SELECT {} 
                FROM videos
            ", fields);      
            let result = conn.prep_exec(sql, ()).await.unwrap();

            // Collect result
            let (_ /* conn */, videos) = result.map_and_drop(|row| {
                let (id, name, createdat) = mysql_async::from_row::<(i32, Vec<u8>, Option<NaiveDateTime>)>(row);
                Video {
                    id: id,
                    name: String::from_utf8_lossy(&name).into_owned(),
                    createdat: Some(createdat.unwrap().to_string()),
                }
            }).await.unwrap();

            videos
        }
    }
}