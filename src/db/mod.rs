use std::collections::HashMap;
use std::env;

pub mod models;
pub mod schema;

use diesel::{
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};

pub struct StateMgr {
    pub connection: Pool<ConnectionManager<SqliteConnection>>,
    pub stop_words: Vec<String>,
    pub glaff: Option<HashMap<String, String>>,
}

pub fn establish_connection() -> Pool<ConnectionManager<SqliteConnection>> {
    dotenvy::dotenv().ok();
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool")
}

pub fn add_keywords_to_database(
    new_keywords: Vec<String>,
    connection: SqliteConnection,
) {
    use schema::keywords::dsl::*;
    for keyword in new_keywords {
        match keywords.find(keyword).first(connection) {
            Some(e) => return,
            None => return,
        }
    }
}
