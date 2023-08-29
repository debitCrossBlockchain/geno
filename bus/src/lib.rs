extern crate threadpool;

use std::time::Duration;
use threadpool::ThreadPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use bytes::Bytes;
use std::fmt::{Formatter, Error, Debug};
use crossbeam_channel::{unbounded, Receiver, Sender};

#[cfg(test)]
mod test;

/// Subscription to a pub/sub channel
pub struct Subscription {
    bus: Bus,
    channel_id: String,
}

impl Debug for Subscription {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        fmt.write_str(&format!("Sub(channel={})", self.channel_id))
    }
}

impl Subscription {
    pub fn cancel(self) { /* self is dropped */
    }

    pub fn notify_others(&self, msg: Bytes) {
        self.bus
            .notify_exception(&self.channel_id, msg);
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.bus.unregister(self);
    }
}

pub struct SubActivator{
	sub: Subscription
}
impl SubActivator{
	pub fn activate<F>(self, func: F) -> Subscription
	where F: FnMut(Bytes) + 'static + Send{
		self.sub.bus.activate(&self.sub.channel_id, func);
		self.sub
	}
}

struct SubMessage {
    running: RAIIBool,
    receiver: Receiver<Bytes>,
    sender:Sender<Bytes>,
    func: Option<Arc<Mutex<Box<dyn FnMut(Bytes)+Send>>>>
}

#[derive(Clone)]
struct RAIIBool {
    value: Arc<Mutex<bool>>,
}
impl RAIIBool {
    fn new(value: bool) -> RAIIBool {
        RAIIBool {
            value: Arc::new(Mutex::new(value)),
        }
    }
    fn set(&self, value: bool) -> bool {
        let mut guard = self.value.lock().unwrap();
        let old: bool = *guard;
        *guard = value;
        old
    }
    fn new_guard(&self, value: bool) -> RAIIBoolGuard {
        RAIIBoolGuard::new(self.clone(), value)
    }
}

struct RAIIBoolGuard {
    data: RAIIBool,
    value: bool,
}
impl RAIIBoolGuard {
    fn new(data: RAIIBool, value: bool) -> RAIIBoolGuard {
        RAIIBoolGuard {
            data: data,
            value: value,
        }
    }
    fn done(self) {}
}
impl Drop for RAIIBoolGuard {
    fn drop(&mut self) {
        self.data.set(self.value);
    }
}

struct InnerBus{
	channels: HashMap<String, SubMessage>,
	//id will stay unique for hundreds of years, even at ~1 billion/sec
	thread_pool: Arc<ThreadPool>
}

unsafe impl Send for InnerBus{}
unsafe impl Sync for InnerBus{}

#[derive(Clone)]
pub struct Bus{
	inner: Arc<Mutex<InnerBus>>
}
unsafe impl Send for Bus{}
unsafe impl Sync for Bus{}

impl Bus{
	pub fn new(num_threads: usize) -> Bus{
		Bus{
			inner: Arc::new(Mutex::new(InnerBus{
				channels: HashMap::new(),
				thread_pool: Arc::new(ThreadPool::new(num_threads))
			}))
		}
	}
	fn internal_subscribe<F>(&self, channel: &str, func: Option<F>) -> Subscription
	where F: FnMut(Bytes)+'static + Send{
        let (send, recv) = unbounded();
		let sub_message = SubMessage{
			running: RAIIBool::new(false),
            receiver: recv,
            sender:send,
			func: func.map(|f|Arc::new(Mutex::new(Box::new(f) as Box<_>)))
		};

		let mut data = self.inner.lock().unwrap();
		if !data.channels.contains_key(channel){
			data.channels.insert(channel.to_string(), sub_message);
		}
		
		let subscriptions = data.channels.get_mut(channel).unwrap();
	
		Subscription{
			bus: self.clone(),
			channel_id: channel.to_string(),
		}
	}

	pub fn subscribe<F>(&self, channel: &str, func: F) -> Subscription
	where F: FnMut(Bytes)+'static + Send{
		self.internal_subscribe(channel, Some(func))
	}
	
	#[allow(unused_assignments)]
	pub fn lazy_subscribe(&self, channel: &str) -> SubActivator{
		let mut func = Some(|_|{});//used to give type info to 'func'
		func = None;
        SubActivator{
            sub: self.internal_subscribe(channel, func),
        }		
	}

	fn activate<F>(&self, channel: &str,  func: F)
	where F: FnMut(Bytes)+'static + Send{
		let mut inner = self.inner.lock().unwrap();
		let pool = inner.thread_pool.clone();
		let sub_message = inner.channels.get_mut(channel).unwrap();//channel will always exist
		sub_message.func = Some(Arc::new(Mutex::new(Box::new(func))));
		self.schedule_worker(sub_message, channel,&pool);
	}
	pub fn num_channels(&self) -> usize{
		let data = self.inner.lock().unwrap();
		data.channels.len()
	}
	fn unregister(&self, sub: &Subscription){
		let mut inner = self.inner.lock().unwrap();
		inner.channels.remove(&sub.channel_id);	
	}
	fn schedule_worker(&self, sub_message: &mut SubMessage, channel: &str,pool: &Arc<ThreadPool>){
		if !sub_message.running.set(true){//if not currently running
			let thread_running = sub_message.running.clone();
			if let Some(func) = sub_message.func.clone(){
                let bus = self.clone();
				let channel = channel.to_string();
				pool.execute(move ||{	
					use std::ops::DerefMut;
					let finish_guard = thread_running.new_guard(false);
					let mut guard = func.lock().unwrap();
					let mut func = guard.deref_mut();
					let mut running = true;
					while running{
						let mut notification_message: Bytes = Bytes::new();
						{  
                            let mut inner = bus.inner.lock().unwrap();
							if let Some(subs) = inner.channels.get_mut(&channel){
								if let Ok(msg) = subs.receiver.try_recv() {
                                            notification_message.clone_from(&msg);
                                   }    
							}
						}//unlock 'inner'
						if !notification_message.is_empty(){
							func(notification_message);
						}else{
							running = false;
						}
					}
					finish_guard.done();
				});
			}else{
				thread_running.set(false);
			}
		}
	}
	pub fn notify(&self, channel: &str, msg: Bytes){
		self.notify_exception(channel, msg)
	}

	fn notify_exception(&self, channel: &str, msg: Bytes){
		let mut inner = self.inner.lock().unwrap();
		let pool = inner.thread_pool.clone();
		if let Some(sub_message) = inner.channels.get_mut(channel){
			let _ = sub_message.sender.send(msg.clone());
			self.schedule_worker(sub_message, channel,  &pool);
		}
	}
}



