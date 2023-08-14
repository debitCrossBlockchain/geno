use utils::logger::{LogInstance, LogUtil};

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static JEMALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
    let _guard = LogUtil::init("./log", "app.log", "setting/log_filter.txt").unwrap();
    let log_instance = LogInstance::new();

    let info = format!(
        "branch={} commit={} source_timestamp={}",
        env!("GIT_BRANCH"),
        env!("GIT_COMMIT"),
        env!("SOURCE_TIMESTAMP"),
    );
}
