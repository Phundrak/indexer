use tracing_subscriber::FmtSubscriber;
use tracing::Level;

pub mod kwparser;

pub fn setup_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed");
}
