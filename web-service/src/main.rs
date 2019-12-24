use {
    actix_web::{middleware, web, http, App, HttpServer},
    std::{env, net::SocketAddr},
    web_service::video_client::VideoClient,
    tera::Tera,
    env_logger,
    mysql_async::Pool,
    web_service::video_service::VideoService,
};

mod api;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    let addr = env::args().nth(1).unwrap_or("localhost:8093".to_string());
    println!("Server is running on http://{}", addr);

    // address of video-service
    let remote_adr: SocketAddr = env::args()
        .nth(2)
        .unwrap_or("127.0.0.1:8091".into())
        .parse()
        .expect("Remote adress structure is not valid");

    // create new client
    let video_client = web::Data::new(VideoClient::new(remote_adr));

    // connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or("mysql://gusyara:12345@localhost:3306/gusyara".to_string());
    let pool = Pool::new(database_url);
    // create new service
    let video_service = web::Data::new(VideoService::new(pool));

    HttpServer::new(move || {

        let tera =
        Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")).unwrap();

        // all errors from web server will be wrapped by this handlers
        let error_handlers = middleware::errhandlers::ErrorHandlers::new()
            .handler(
                http::StatusCode::INTERNAL_SERVER_ERROR,
                api::internal_server_error,
            )
            .handler(http::StatusCode::BAD_REQUEST, api::bad_request)
            .handler(http::StatusCode::NOT_FOUND, api::not_found);

        App::new()
            .data(tera)
            .wrap(middleware::Logger::default())
            .wrap(error_handlers)
            .service(
                web::scope("/video")
                    // dependency injection of video client
                    .app_data(video_client.clone())
                    .app_data(video_service.clone())
                    .route("/upload", web::post().to(api::save_file))
                    .route("/", web::get().to(api::show_video))
                    .route("/{filename}", web::get().to(api::get_file)))
            .service(
                web::resource("/")
                    .route(web::get().to(api::index)),   
            )
    })
    .bind(addr)?
    .start()
    .await
}