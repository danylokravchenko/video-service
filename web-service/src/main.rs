#[macro_use]
extern crate diesel;
#[macro_use]
extern crate serde_derive;

use {
    actix_web::{middleware, web, http, App, HttpServer},
    std::{env, net::SocketAddr},
    web_service::video_client::VideoClient,
    tera::Tera,
    env_logger,
    dotenv,
};

mod api;
mod models;
mod schema;
mod db;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // init logging
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    // read env variables from `.env`
    dotenv::dotenv().ok();

    let addr = env::args().nth(1).unwrap_or("localhost:8090".to_string());
    println!("Server is running on http://{}", addr);

    // address of video-service
    let remote_adr: SocketAddr = env::args()
        .nth(2)
        .unwrap_or("127.0.0.1:8091".into())
        .parse()
        .expect("Remote adress structure is not valid");

    // create new client
    let video_client = web::Data::new(VideoClient::new(remote_adr));

    // connect to database and create pool of connections
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = db::init_pool(&database_url).expect("Failed to create pool");

    // create new HTTPServer with dependency injections and custom wrappers
    HttpServer::new(move || {

        // create template engine and register folder where to look for defined templates
        let tera =
        Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")).unwrap();

        // all errors from web server will be wrapped by those handlers
        let error_handlers = middleware::errhandlers::ErrorHandlers::new()
            .handler(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                api::internal_server_error,
            )
            .handler(http::StatusCode::BAD_REQUEST, api::bad_request)
            .handler(http::StatusCode::NOT_FOUND, api::not_found);

        App::new()
            // inject template engine
            .data(tera)
            // inject database pool
            .data(pool.clone())
            .wrap(middleware::Logger::default())
            .wrap(error_handlers)
            // group routes by `video` into a service
            .service(
                web::scope("/video")
                    // dependency injection of video client
                    .app_data(video_client.clone())
                    .route("/upload", web::post().to(api::save_file))
                    .route("/", web::get().to(api::list_videos))
                    .route("/show", web::get().to(api::show_video))
                    .route("/file/{filename}", web::get().to(api::get_file)))        
            .service(
                web::resource("/")
                    .route(web::get().to(api::index)),   
            )
    })
    .bind(addr)?
    .run()
    .await
}