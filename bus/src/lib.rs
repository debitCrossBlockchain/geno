extern crate threadpool;

use threadpool::ThreadPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use bytes::Bytes;
use std::fmt::{Formatter, Error, Debug};
use crossbeam_channel::{unbounded, Receiver, Sender};

/// Subscription to a pub/sub channel
pub struct Subscription {
    bus: Bus,
    channel_id: String,
    id: u64,
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
            .notify_exception(&self.channel_id, msg, Some(self.id));
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.bus.unregister(self);
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
	channels: HashMap<String, HashMap<u64, SubMessage>>,
	//id will stay unique for hundreds of years, even at ~1 billion/sec
	next_id: u64,
	thread_pool: Arc<ThreadPool>
}
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
				next_id: 0,
				thread_pool: Arc::new(ThreadPool::new(num_threads))
			}))
		}
	}
	fn internal_subscribe<F>(&self, channel: &str, func: Option<F>) -> Subscription
	where F: FnMut(Bytes)+'static + Send{
		let mut data = self.inner.lock().unwrap();
		if !data.channels.contains_key(channel){
			data.channels.insert(channel.to_string(), HashMap::new());
		}
		let id = data.next_id;
		data.next_id += 1;
        let (send, recv) = unbounded();
		let sub_message = SubMessage{
			running: RAIIBool::new(false),
            receiver: recv,
            sender:send,
			func: func.map(|f|Arc::new(Mutex::new(Box::new(f) as Box<_>)))
		};
		
		let subscriptions = data.channels.get_mut(channel).unwrap();
		subscriptions.insert(id, sub_message);
		Subscription{
			bus: self.clone(),
			channel_id: channel.to_string(),
			id: id,
		}
	}

	pub fn subscribe<F>(&self, channel: &str, func: F) -> Subscription
	where F: FnMut(Bytes)+'static + Send{
		self.internal_subscribe(channel, Some(func))
	}
	
	#[allow(unused_assignments)]
	pub fn lazy_subscribe(&self, channel: &str) -> Subscription{
		let mut func = Some(|_|{});//used to give type info to 'func'
		func = None;
		self.internal_subscribe(channel, func)	
	}

	fn activate<F>(&self, channel: &str, id: u64, func: F)
	where F: FnMut(Bytes)+'static + Send{
		let mut inner = self.inner.lock().unwrap();
		let pool = inner.thread_pool.clone();
		let subs = inner.channels.get_mut(channel).unwrap();//channel will always exist
		let sub_message = subs.get_mut(&id).unwrap();//sub id will always exist
		sub_message.func = Some(Arc::new(Mutex::new(Box::new(func))));
		self.schedule_worker(sub_message, channel, id, &pool);
	}
	pub fn num_channels(&self) -> usize{
		let data = self.inner.lock().unwrap();
		data.channels.len()
	}
	fn unregister(&self, sub: &Subscription){
		let mut inner = self.inner.lock().unwrap();
		let mut remove_channel = false;
		{
			let sub_list = inner.channels.get_mut(&sub.channel_id).unwrap();
			sub_list.remove(&sub.id);
			if sub_list.len() == 0{
				remove_channel = true;
			}
		}
		if remove_channel{
			inner.channels.remove(&sub.channel_id);
		}
	}
	fn schedule_worker(&self, sub_message: &mut SubMessage, channel: &str, id: u64, pool: &Arc<ThreadPool>){
		if !sub_message.running.set(true){//if not currently running
			let thread_running = sub_message.running.clone();
			if let Some(func) = sub_message.func.clone(){
				let bus = self.clone();
				let channel = channel.to_string();
				let id = id.clone();
				pool.execute(move ||{	
					use std::ops::DerefMut;
					let finish_guard = thread_running.new_guard(false);
					let mut guard = func.lock().unwrap();
					let mut func = guard.deref_mut();
					let mut running = true;
					while running{
						let mut notification_message: Bytes = Bytes::new();
						{
                            let temp_bus = bus.clone();
							let mut inner = temp_bus.inner.lock().unwrap();
							if let Some(subs) = inner.channels.get_mut(&channel){
								if let Some(sub_message) = subs.get_mut(&id){
                                   if let Ok(msg) = sub_message.receiver.recv() {
                                            notification_message.clone_from(&msg);
                                   }    
								}
							}
						}//unlock 'inner'
						if  notification_message.len()!=0{
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
		self.notify_exception(channel, msg, None)
	}
	fn notify_exception(&self, channel: &str, msg: Bytes, exception: Option<u64>){
		let mut inner = self.inner.lock().unwrap();
		let pool = inner.thread_pool.clone();
		if let Some(subscriptions) = inner.channels.get_mut(channel){
			for (id,sub_message) in subscriptions{
				if Some(*id) != exception{
					sub_message.sender.send(msg.clone());
					self.schedule_worker(sub_message, channel, *id, &pool);
				}
			}
		}
	}
}



