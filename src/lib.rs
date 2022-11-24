#![warn(clippy::style, clippy::pedantic)]
#![allow(clippy::no_effect_underscore_binding)]

#[macro_use]
extern crate rocket;

use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub mod spelling;
pub mod server;
pub mod db;
pub mod kwparser;
pub mod fileparser;

pub fn setup_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed");
}
