use web_service::video_client::VideoClient;
use actix_multipart::Multipart;
use actix_web::{web, http, Error, HttpResponse};
use futures::StreamExt;
use std::io::Write;
use std::net::SocketAddr;
use bytes::Bytes;

pub async fn save_file((mut payload, video_client): (Multipart, web::Data<VideoClient>)) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    while let Some(item) = payload.next().await {
        let mut video_conn = video_client.conn()
            .await
            .expect("Can not connect to remote video-service");

        let mut field = item?;
        let content_type = field.content_disposition().unwrap();
        let filename = content_type.get_filename().unwrap();
        video_conn.send_filename(&filename).await.unwrap();

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