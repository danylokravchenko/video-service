use {
    web_service::video_client::VideoClient,
    actix_multipart::Multipart,
    actix_web::{web, http, error, HttpResponse},
    futures::StreamExt,
    failure::Fail,
};

#[derive(Fail, Debug)]
#[fail(display = "video client error")]
pub enum VideoError {
    #[fail(display = "An internal error occurred. Err: {}. Please try again later", msg)]
    InternalError {msg: String},
    // #[fail(display = "bad request")]
    // BadClientData,
    // #[fail(display = "timeout")]
    // Timeout,
}

impl error::ResponseError for VideoError {
    // fn error_response(&self) -> HttpResponse {
    //     match *self {
    //         VideoError::InternalError {msg} => {
    //             HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR)
    //         }
    //     }
    // }
}

pub async fn save_file((mut payload, video_client): (Multipart, web::Data<VideoClient>)) -> Result<HttpResponse, VideoError> {
    // iterate over multipart stream
    while let Some(item) = payload.next().await {
        let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

        let mut field = item.unwrap();
        let content_type = field.content_disposition().unwrap();
        // TODO: generate unique filename,
        // don't trust this incomming filename
        let filename = content_type.get_filename().unwrap();
        match video_conn.start_uploading(&filename).await {
            Err(e) => {
                println!("{}", e);
                return Err(VideoError::InternalError{msg: e.to_string()});
            },
            Ok(_) => {
                println!("uploading a file");
            },
        };

        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            video_conn.buffered_send(data).await.unwrap();
        }
        video_conn.flush().await.unwrap();
    }
    Ok(redirect_to("/"))
}

pub fn index() -> HttpResponse {
    let html = r#"<html>
        <head><title>Upload Test</title></head>
        <body>
            <form action="/video/upload" method="post" enctype="multipart/form-data">
                <input type="file" multiple name="file"/>
                <input type="submit" value="Submit"></button>
            </form>
        </body>
    </html>"#;
    HttpResponse::Ok().body(html)
}

// redirects to specific path
fn redirect_to(location: &str) -> HttpResponse {
    HttpResponse::Found()
        .header(http::header::LOCATION, location)
        .finish()
}