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
    serde::Deserialize,
    crate::db,
};

// custom errors
#[derive(Fail, Debug)]
#[fail(display = "video client error")]
pub enum VideoError {
    #[fail(display = "An internal error occurred. Err: {}. Please try again later", msg)]
    InternalError {msg: String},
    #[fail(display = "Vido file was not found. Err: {}", msg)]
    NotFound {msg: String},
}

// implement trait for custom VideoError to use it as actix-web error
impl error::ResponseError for VideoError {
    fn error_response(&self) -> HttpResponse {
        match self {
            VideoError::InternalError{msg} => HttpResponse::InternalServerError().json(msg),
            VideoError::NotFound{msg} => HttpResponse::NotFound().json(msg),
        }
    }
}

// render upload file form 
pub async fn index(tmpl: web::Data<Tera>) -> Result<HttpResponse, Error> {
    let s = tmpl.render("index.html", &tera::Context::new())
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// upload video file in chunks to remote video service
pub async fn save_file(
    (mut payload, video_client, pool): 
    (Multipart, web::Data<VideoClient>, web::Data<db::MysqlPool>)
) -> Result<HttpResponse, Error> {
    let mut files = Vec::new();
    // iterate over multipart stream
    while let Some(item) = payload.next().await {
        // establish connection to remote video service
        let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

        let mut field = item.unwrap();
        // generate random filename
        let filename: Vec<u32> = thread_rng()
            .sample_iter(&Standard)
            .take(1)
            .collect();
        // only need first 7 chars for now
        let filename = filename[0].to_string()[0..7].to_string() + ".mp4";
        video_conn.start_uploading(&filename).await
            .map_err(|e| VideoError::InternalError{msg: e.to_string()})?;

        // send each chunk to remote video service
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            video_conn.buffered_send(data).await.unwrap();
        }
        // flush buffer and send rest of bytes
        video_conn.flush().await.unwrap();

        files.push(filename);
        
    }
    // insert uploaded videos to the database
    web::block(move || db::insert_many_videos(files, &pool)).await?;
    
    Ok(redirect_to("/"))
}

#[derive(Deserialize)]
pub struct Info {
    pub id: i32,
}

// render videoplayer
pub async fn show_video(
    (tmpl, query, pool): 
    (web::Data<Tera>, web::Query<Info>, web::Data<db::MysqlPool>)
) -> Result<HttpResponse, Error> {
    // check if video exist
    let video = match web::block(move || db::get_video(query.id, &pool)).await {
        Err(_) => {
            return Err(VideoError::NotFound{msg: format!("Video was not found")})?;
        },
        Ok(video) => video,
    };
    // pass data from populated video into template
    let mut ctx = tera::Context::new();
    ctx.insert("name", &video.name);
    let s = tmpl.render("video.html", &ctx)
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// get video file from remote video service
pub async fn get_file(
    (filename, video_client): 
    (web::Path<String>, web::Data<VideoClient>)
) -> Result<HttpResponse, Error> {

    let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

    video_conn.start_receiving(&filename).await
        .map_err(|e| VideoError::NotFound{msg: e.to_string()})?;
    
    let (tx, rx_body) = mpsc::unbounded();

    // spawn new task to receive video in many parts and push in stream
    actix::spawn(async move {
        while let Some(chunk) = video_conn.read_next().await {
            tx.unbounded_send(chunk).unwrap();
        }
    });
    // response with HTTP Streaming body
    Ok(HttpResponse::Ok().streaming(rx_body))
}

// list all available videos
pub async fn list_videos(
    (tmpl, pool): 
    (web::Data<Tera>, web::Data<db::MysqlPool>)
) -> Result<HttpResponse, Error> {
    let videos = web::block(move || db::get_all_videos(&pool)).await?;
    let mut ctx = tera::Context::new();
    ctx.insert("videos", &videos);
    let s = tmpl.render("list.html", &ctx)
            .map_err(|_| error::ErrorInternalServerError("Template error"))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

// redirect to specific path
fn redirect_to(location: &str) -> HttpResponse {
    HttpResponse::Found()
        .header(http::header::LOCATION, location)
        .finish()
}

// custom error handlers will wrap error and 
// return specific static html that represents this error

// custom handler for 400 BadRequest Error
pub fn bad_request<B>(res: dev::ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    let new_resp = NamedFile::open("static/errors/400.html")?
        .set_status_code(res.status())
        .into_response(res.request())?;
    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_resp.into_body()),
    ))
}

// custom handler for 404 NotFound Error
pub fn not_found<B>(res: dev::ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    let new_resp = NamedFile::open("static/errors/404.html")?
        .set_status_code(res.status())
        .into_response(res.request())?;
    Ok(ErrorHandlerResponse::Response(
        res.into_response(new_resp.into_body())
        )
    )
}

// custom handler for 500 InternalServer Error
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