use anyhow::{anyhow, Context as _, Result};
use hotwatch::{Event, Hotwatch};
use std::{fs, path::Path};
use time::macros::format_description;
use tracing::*;
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_local_time::LocalTime;
use tracing_subscriber::{fmt, layer::SubscriberExt, reload::Handle, EnvFilter};

pub struct LogUtil;
impl LogUtil {
    /// Inits log.
    /// Returns a WorkerGuard to ensure buffered logs are flushed,
    ///  and a Hotwatch to watch the log filter file.
    pub fn init(
        directory: impl AsRef<Path>,
        file_name_prefix: impl AsRef<Path>,
        log_filter_file: impl AsRef<Path>,
    ) -> Result<(WorkerGuard, Hotwatch)> {
        let timer = LocalTime::new(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
        ));

        let file_appender = rolling::daily(directory, file_name_prefix);
        let (non_blocking, worker_guard) = tracing_appender::non_blocking(file_appender);
        let file_layer = fmt::Layer::default()
            .with_writer(non_blocking)
            .with_line_number(true)
            .with_timer(timer.clone())
            .json()
            .flatten_event(true)
            .with_ansi(false);

        let builder = tracing_subscriber::fmt()
            .with_timer(timer)
            .with_env_filter(EnvFilter::from_default_env())
            .with_filter_reloading();
        let handle = builder.reload_handle();
        let subscriber = builder.finish();
        let subscriber = subscriber.with(file_layer);
        tracing::subscriber::set_global_default(subscriber)
            .context("set global default subscriber")?;

        Self::reload_filter(handle.clone(), log_filter_file.as_ref());

        let log_filter_path_buf = log_filter_file.as_ref().to_path_buf();
        let mut hotwatch = Hotwatch::new().context("hotwatch failed to initialize!")?;
        hotwatch
            .watch(log_filter_file.as_ref(), move |event: Event| {
                debug!("log filter file event: {:?}", event);
                if let Event::Write(_) = event {
                    Self::reload_filter(handle.clone(), log_filter_path_buf.clone());
                }
            })
            .with_context(|| format!("failed to watch file: {:?}", log_filter_file.as_ref()))?;

        Ok((worker_guard, hotwatch))
    }

    fn reload_filter<S: Subscriber + 'static>(
        handle: Handle<EnvFilter, S>,
        log_filter_file: impl AsRef<Path>,
    ) {
        let res = Self::try_reload_filter(handle, log_filter_file);
        match res {
            Ok(_) => debug!("reload log filter OK"),
            Err(e) => {
                warn!("reload log filter error: {:?}", e)
            }
        }
    }

    fn try_reload_filter<S: Subscriber + 'static>(
        handle: Handle<EnvFilter, S>,
        log_filter_file: impl AsRef<Path>,
    ) -> Result<()> {
        let contents = fs::read_to_string(log_filter_file.as_ref()).with_context(|| {
            format!(
                "something went wrong reading the file: {:?}",
                log_filter_file.as_ref()
            )
        })?;
        let contents = contents.trim();
        debug!("reload log filter: {:?}", contents);
        let new_filter = contents
            .parse::<EnvFilter>()
            .map_err(|e| anyhow!(e))
            .context("failed to parse env filter")?;
        handle.reload(new_filter).context("handle reload error")
    }

    // FIXME: this can probably be done more elegantly
    /// Creates the node's tracing span based on its name.
    pub fn create_span(name: &str) -> Span {
        let mut span = trace_span!("node", name = name);
        if !span.is_disabled() {
            return span;
        } else {
            span = debug_span!("node", name = name);
        }
        if !span.is_disabled() {
            return span;
        } else {
            span = info_span!("node", name = name);
        }
        if !span.is_disabled() {
            return span;
        } else {
            span = warn_span!("node", name = name);
        }
        if !span.is_disabled() {
            span
        } else {
            error_span!("node", name = name)
        }
    }
}
