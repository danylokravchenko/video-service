use {
    diesel,
    diesel::mysql::MysqlConnection,
    diesel::prelude::*,
    chrono::{Utc, NaiveDateTime},
    crate::schema::{
        videos,
        videos::dsl::{videos as all_videos},
    },
};

#[derive(PartialEq, Clone, Debug, Queryable, Serialize, Deserialize)]
pub struct Video {
    pub id: i32,
    pub name: String,
    #[serde(with = "my_date_format")]
    pub createdat: Option<NaiveDateTime>,
}

mod my_date_format {
    use chrono::NaiveDateTime;
    use serde::{self, Deserialize, Serializer, Deserializer};

    const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";

    // The signature of a serialize_with function must follow the pattern:
    //
    //    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
    //    where
    //        S: Serializer
    //
    // although it may also be generic over the input types T.
    pub fn serialize<S>(
        date: &Option<NaiveDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.unwrap().format(FORMAT));
        serializer.serialize_str(&s)
    }

    // The signature of a deserialize_with function must follow the pattern:
    //
    //    fn deserialize<'de, D>(D) -> Result<T, D::Error>
    //    where
    //        D: Deserializer<'de>
    //
    // although it may also be generic over the output types T.
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Some(NaiveDateTime::parse_from_str(&s, FORMAT).unwrap()))
    }
}

#[derive(Insertable)]
#[table_name = "videos"]
pub struct NewVideo {
    pub name: String,
    pub createdat: Option<NaiveDateTime>,
}

impl NewVideo {
    pub fn new(name: String) -> NewVideo {
        NewVideo { 
            name: name, 
            createdat: Some(Utc::now().naive_utc()) 
        }
    }
}

impl Video {
    pub fn all(conn: &MysqlConnection) -> QueryResult<Vec<Video>> {
        all_videos.order(videos::createdat.desc()).load::<Video>(conn)
    }

    pub fn insert(video: NewVideo, conn: &MysqlConnection) -> QueryResult<usize> {
        diesel::insert_into(videos::table)
            .values(&video)
            .execute(conn)
    }

    pub fn insert_many(videos: Vec<NewVideo>, conn: &MysqlConnection) -> QueryResult<usize> {
        diesel::insert_into(videos::table)
            .values(&videos)
            .execute(conn)
    }

    pub fn find_by_id(id: i32, conn: &MysqlConnection) -> QueryResult<Video> {
        all_videos.find(id).get_result::<Video>(conn)
    }

    pub fn delete_with_id(id: i32, conn: &MysqlConnection) -> QueryResult<usize> {
        diesel::delete(all_videos.find(id)).execute(conn)
    }
}