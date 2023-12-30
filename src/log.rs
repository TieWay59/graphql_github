use chrono::SecondsFormat;
pub use log::*;

pub static MY_LOGGER: MyLogger = MyLogger;

pub struct MyLogger;

impl log::Log for MyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!(
                "{} [{}] {}",
                chrono::Local::now().to_rfc3339_opts(SecondsFormat::Millis, false),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
