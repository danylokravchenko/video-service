use {
    web_service::video_client::VideoClient,
    actix_multipart::Multipart,
    actix_web::{web, http, error, Error, HttpResponse},
    futures::{StreamExt, channel::mpsc},
    failure::Fail,
    tera::Tera,
    actix,
};

#[derive(Fail, Debug)]
#[fail(display = "video client error")]
pub enum VideoError {
    #[fail(display = "An internal error occurred. Err: {}. Please try again later", msg)]
    InternalError {msg: String},
    #[fail(display = "Vido file was not found. Err: {}. Please try again later", msg)]
    NotFound {msg: String},
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

// render upload file form 
pub async fn index(tmpl: web::Data<Tera>) -> Result<HttpResponse, Error> {
    let s = tmpl.render("index.html", &tera::Context::new())
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// upload video file to remote video service
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
        video_conn.start_uploading(&filename).await
            .map_err(|e| VideoError::InternalError{msg: e.to_string()})?;

        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            video_conn.buffered_send(data).await.unwrap();
        }
        video_conn.flush().await.unwrap();
    }
    Ok(redirect_to("/"))
}

// render videoplayer
pub async fn show_video(tmpl: web::Data<Tera>) -> Result<HttpResponse, Error> {
    let filename = "test.mp4";
    let mut ctx = tera::Context::new();
    ctx.insert("name", &filename.to_owned());
    let s = tmpl.render("video.html", &ctx)
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}


// get video file from video service
pub async fn get_file((filename, video_client): (web::Path<String>, web::Data<VideoClient>)) -> Result<HttpResponse, VideoError> {
    let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

    video_conn.start_recieving(&filename).await
        .map_err(|e| VideoError::NotFound{msg: e.to_string()}).unwrap();
    
    let (tx, rx_body) = mpsc::unbounded();

    // spawn new task to recieve video in many parts
    actix::spawn(async move {
        while let Some(chunk) = video_conn.read_next().await {
            tx.unbounded_send(chunk).unwrap();
        }
    });
    
    Ok(HttpResponse::Ok().streaming(rx_body))
}

// redirects to specific path
fn redirect_to(location: &str) -> HttpResponse {
    HttpResponse::Found()
        .header(http::header::LOCATION, location)
        .finish()
}