use crate::timer;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use tracing::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TimerEventType {
    //repeat timer
    LedgerSync,
    //repeat timer
    LedgerUpgrade,
    //repeat timer
    PbftConsensusCheck,
    //delay timer
    PbftConsensusPublish,
    //delay timer,80s
    PbftLedgerCloseCheck,
    //delay timer 30s
    PbftNewViewRepond,
    // delay timer,5s
    PbftDelayDeleteTx,
    // Tbft TimeOut
    TbftTimeOutCheck,
}

pub static GLOBAL_TIMER: Lazy<Mutex<timer::Timer>> = Lazy::new(|| {
    let timer = timer::Timer::new();
    Mutex::new(timer)
});

pub struct TimterEventParam {
    pub id: i64,
    pub event_type: TimerEventType,
    pub data: Option<Vec<u8>>,
    pub timestamp: i64,
}

// pub type TimerSender = tokio::sync::mpsc::UnboundedSender<TimterEventParam>;
pub type TimerSender = crossbeam_channel::Sender<TimterEventParam>;
pub struct TimterItem {
    guard: timer::Guard,
    sender: TimerSender,
}
pub struct TimerManager {
    pub counter: AtomicI64,
    pub pool: RwLock<HashMap<i64, TimterItem>>,
}

pub static TIMER_MANAGER_INSTANCE: OnceCell<TimerManager> = OnceCell::new();

impl TimerManager {
    pub fn instance() -> &'static TimerManager {
        TIMER_MANAGER_INSTANCE
            .get()
            .expect("TimerManager is not initialized")
    }

    pub fn new() -> TimerManager {
        TimerManager {
            counter: AtomicI64::new(1),
            pool: RwLock::new(HashMap::new()),
        }
    }

    pub fn delete_timer(&self, i: i64) -> bool {
        if let Some(v) = self.pool.write().remove(&i) {
            drop(v.guard);
            return true;
        }
        false
    }

    pub fn new_delay_timer(
        &self,
        delay: chrono::Duration,
        sender: TimerSender,
        event_type: TimerEventType,
        data: Option<Vec<u8>>,
    ) -> i64 {
        let i = self.counter.fetch_add(1, Ordering::SeqCst);

        let sender_clone = sender.clone();
        let guard = {
            GLOBAL_TIMER.lock().schedule_with_delay(delay, move || {
                let param = TimterEventParam {
                    id: i,
                    event_type,
                    data: data.clone(),
                    timestamp: chrono::Local::now().timestamp_millis(),
                };
                if sender_clone.send(param).is_err() {
                    error!("send timer event error");
                }
            })
        };

        let item = TimterItem { guard, sender };
        self.pool.write().insert(i, item);
        i
    }

    pub fn new_repeating_timer(
        &self,
        duration: chrono::Duration,
        sender: TimerSender,
        event_type: TimerEventType,
        data: Option<Vec<u8>>,
    ) -> i64 {
        let i = self.counter.fetch_add(1, Ordering::SeqCst);

        let sender_clone = sender.clone();
        let guard = {
            GLOBAL_TIMER.lock().schedule_repeating(duration, move || {
                let param = TimterEventParam {
                    id: i,
                    event_type,
                    data: data.clone(),
                    timestamp: chrono::Local::now().timestamp_millis(),
                };
                if sender_clone.send(param).is_err() {
                    error!("send timer event error");
                }
            })
        };

        let item = TimterItem { guard, sender };
        self.pool.write().insert(i, item);
        i
    }
}

pub fn initialize_timer_manager() {
    let timer_menager = TimerManager::new();
    let _ = TIMER_MANAGER_INSTANCE.set(timer_menager);
}

//
//
// Example
//
// extern crate anyhow;
// extern crate chrono;
// use crate::timer_manager::{TimerManager, TIMER_MANAGER_INSTANCE};
// use anyhow::Result;
// use smol::Timer;
// use tokio::sync::mpsc;
//
// #[tokio::main]
// async fn main() -> Result<()> {
//     initialize_timer_manager();

//     let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

//     let tid_1 = TimerManager::instance().new_delay_timer(chrono::Duration::seconds(3), tx.clone());
//     println!("create delay timer {}", tid_1);

//     let tid_2 =
//         TimerManager::instance().new_repeating_timer(chrono::Duration::seconds(1), tx.clone());
//     println!("create timer {}", tid_2);

//     tokio::spawn(consume(rx));

//     Timer::after(std::time::Duration::from_secs(600)).await;

//     TimerManager::instance().delete_timer(tid_2);

//     Timer::after(std::time::Duration::from_secs(30)).await;

//     println!("exit");
//     Ok(())
// }

// async fn consume(mut events: mpsc::UnboundedReceiver<i64>) {
//     while let Some(id) = events.recv().await {
//         println!("recv timer id({})", id);
//     }
// }
//
//
