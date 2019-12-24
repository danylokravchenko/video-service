extern crate rand;
extern crate chrono;

use {
    web_service::video_client::VideoClient,
    actix_multipart::Multipart,
    actix_web::{web, dev, http, error, Error, HttpResponse, Result},
    actix_web::middleware::errhandlers::ErrorHandlerResponse,
    actix_files::NamedFile,
    futures::{StreamExt, channel::mpsc},
    failure::Fail,
    tera::Tera,
    actix,
    rand::{thread_rng, Rng},
    rand::distributions::Standard,
    web_service::video_service::{VideoService},
    serde::Deserialize,
};

#[derive(Fail, Debug)]
#[fail(display = "video client error")]
pub enum VideoError {
    #[fail(display = "An internal error occurred. Err: {}. Please try again later", msg)]
    InternalError {msg: String},
    #[fail(display = "Vido file was not found. Err: {}", msg)]
    NotFound {msg: String},
}

// leave the default implementation
impl error::ResponseError for VideoError {}

// render upload file form 
pub async fn index(tmpl: web::Data<Tera>) -> Result<HttpResponse, Error> {
    let s = tmpl.render("index.html", &tera::Context::new())
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// upload video file to remote video service
pub async fn save_file((mut payload, video_client, video_service): (Multipart, web::Data<VideoClient>, web::Data<VideoService>)) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    while let Some(item) = payload.next().await {
        let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

        let mut field = item.unwrap();
        // generate random filename
        let filename: Vec<u32> = thread_rng()
            .sample_iter(&Standard)
            .take(1)
            .collect();
        let filename = filename[0].to_string()[0..7].to_string() + ".mp4";
        video_conn.start_uploading(&filename).await
            .map_err(|e| VideoError::InternalError{msg: e.to_string()})?;

        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            video_conn.buffered_send(data).await.unwrap();
        }
        video_conn.flush().await.unwrap();

        // insert new video to the database
        video_service.insert_video(&filename).await.unwrap();
    }
    Ok(redirect_to("/"))
}

#[derive(Deserialize)]
pub struct Info {
    pub id: i32,
}
// render videoplayer
pub async fn show_video((tmpl, query, video_service): (web::Data<Tera>, web::Query<Info>, web::Data<VideoService>)) -> Result<HttpResponse, Error> {
    let video = match video_service.find_video(query.id).await {
        None => {
            return Err(error::ErrorNotFound(VideoError::NotFound{msg: format!("Video was not found")}));
        },
        Some(video) => video,
    };
    let mut ctx = tera::Context::new();
    ctx.insert("name", &video.name);
    let s = tmpl.render("video.html", &ctx)
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// get video file from video service
pub async fn get_file((filename, video_client): (web::Path<String>, web::Data<VideoClient>)) -> Result<HttpResponse, Error> {
    let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

    video_conn.start_recieving(&filename).await
        .map_err(|e| error::ErrorNotFound(VideoError::NotFound{msg: e.to_string()}))?;
    
    let (tx, rx_body) = mpsc::unbounded();

    // spawn new task to recieve video in many parts
    actix::spawn(async move {
        while let Some(chunk) = video_conn.read_next().await {
            tx.unbounded_send(chunk).unwrap();
        }
    });
    
    Ok(HttpResponse::Ok().streaming(rx_body))
}

// list all available videos
pub async fn list_videos((tmpl, video_service): (web::Data<Tera>, web::Data<VideoService>)) -> Result<HttpResponse, Error> {
    let videos = video_service.list_videos().await;
    let mut ctx = tera::Context::new();
    ctx.insert("videos", &videos);
    let s = tmpl.render("list.html", &ctx)
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// redirects to specific path
fn redirect_to(location: &str) -> HttpResponse {
    HttpResponse::Found()
        .header(http::header::LOCATION, location)
        .finish()
}

pub fn bad_request<B>(res: dev::ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    let new_resp = NamedFile::open("static/errors/400.html")?
        .set_status_code(res.status())
        .into_response(res.request())?;
    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_resp.into_body()),
    ))
}

pub fn not_found<B>(res: dev::ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    let new_resp = NamedFile::open("static/errors/404.html")?
        .set_status_code(res.status())
        .into_response(res.request())?;
    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_resp.into_body())
        )
    )
}

pub fn internal_server_error<B>(
    res: dev::ServiceResponse<B>,
) -> Result<ErrorHandlerResponse<B>> {
    let new_resp = NamedFile::open("static/errors/500.html")?
        .set_status_code(res.status())
        .into_response(res.request())?;
    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_resp.into_body()),
    ))
}