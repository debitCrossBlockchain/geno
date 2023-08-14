use anyhow::{anyhow, Context as _, Result};
use hotwatch::{Event, Hotwatch};
use std::{fs, ops::Deref, path::Path, sync::Arc};
use time::{format_description, UtcOffset};
use tracing::*;
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{
    fmt::time::OffsetTime,
    fmt::{self, time::LocalTime, writer::MakeWriterExt},
    layer::SubscriberExt,
    reload::Handle,
    EnvFilter,
};

pub struct LogItem {
    ledger_span: Arc<Span>,
    network_span: Arc<Span>,
    consensus_span: Arc<Span>,
    txpool_span: Arc<Span>,
}

/// PeerNetwork
#[derive(Clone)]
pub struct LogInstance(Arc<LogItem>);

impl Deref for LogInstance {
    type Target = Arc<LogItem>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LogInstance {
    pub fn new() -> LogInstance {
        LogInstance {
            0: Arc::new(LogItem {
                ledger_span: Arc::new(LogUtil::create_span("ledger")),
                network_span: Arc::new(LogUtil::create_span("network")),
                consensus_span: Arc::new(LogUtil::create_span("consensus")),
                txpool_span: Arc::new(LogUtil::create_span("txpool")),
            }),
        }
    }

    pub fn ledger_span(&self) -> Arc<Span> {
        self.0.ledger_span.clone()
    }

    pub fn network_span(&self) -> Arc<Span> {
        self.0.network_span.clone()
    }

    pub fn consensus_span(&self) -> Arc<Span> {
        self.0.network_span.clone()
    }

    pub fn txpool_span(&self) -> Arc<Span> {
        self.0.network_span.clone()
    }
}
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
        let format = "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]";
        // let local_offset = UtcOffset::current_local_offset().unwrap();
        let local_offset = UtcOffset::from_hms(8, 0, 0).unwrap();
        let time_format = format_description::parse(format).unwrap();
        let timer = OffsetTime::new(local_offset, time_format);

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
}
