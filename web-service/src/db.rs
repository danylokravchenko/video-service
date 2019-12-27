use {
    std::ops::Deref,
    diesel::mysql::MysqlConnection,
    diesel::r2d2::{ConnectionManager, Pool, PoolError, PooledConnection},
    crate::models::{NewVideo, Video},
};

// custom types to simplify code
pub type MysqlPool = Pool<ConnectionManager<MysqlConnection>>;
type MysqlPooledConnection = PooledConnection<ConnectionManager<MysqlConnection>>;

// init pool with database url
pub fn init_pool(database_url: &str) -> Result<MysqlPool, PoolError> {
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    Pool::builder().build(manager)
}

// get new connection from the pool
fn get_conn(pool: &MysqlPool) -> Result<MysqlPooledConnection, &'static str> {
    pool.get().map_err(|_| "Can't get connection")
}

// list all videos
pub fn get_all_videos(pool: &MysqlPool) -> Result<Vec<Video>, &'static str> {
    Video::all(get_conn(pool)?.deref()).map_err(|_| "Error getting all videos")
}

// insert new video
pub fn insert_many_videos(files: Vec<String>, pool: &MysqlPool) -> Result<(), &'static str> {
    
    // transform vector of file names into vector of new videos
    let new_videos = files.into_iter()
        .map(|filename| NewVideo::new(filename))
        .collect::<Vec<NewVideo>>();

    Video::insert_many(new_videos, get_conn(pool)?.deref())
        .map(|_| ())
        .map_err(|_| "Error inserting video")
}

// insert new video
pub fn create_video(filename: String, pool: &MysqlPool) -> Result<(), &'static str> {
    let new_video = NewVideo::new(filename);
    Video::insert(new_video, get_conn(pool)?.deref())
        .map(|_| ())
        .map_err(|_| "Error inserting video")
}

// get video by PK
pub fn get_video(id: i32, pool: &MysqlPool) -> Result<Video, &'static str> {
    Video::find_by_id(id, get_conn(pool)?.deref())
        .map_err(|_| "Video not found")
}

// delete video
pub fn delete_video(id: i32, pool: &MysqlPool) -> Result<(), &'static str> {
    Video::delete_with_id(id, get_conn(pool)?.deref())
        .map(|_| ())
        .map_err(|_| "Error deleting video")
}