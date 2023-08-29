use super::Bus;
use bytes::Bytes;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use rand::{distributions::Standard, rngs::SmallRng, Rng, SeedableRng};
use deadline::deadline;

const NUM_MESSAGES: usize = 10_1000;
const MSG_SIZE: usize = 1024*1024*32;

#[test]
fn basic_test() {
    let bus = Bus::new(5);
    let count = Arc::new(Mutex::new(0));
    {
        let count1 = count.clone();
        let sub1 = bus.subscribe("channel1", move |_| {
            sleep(Duration::from_millis(1000));
            *count1.lock().unwrap() += 1;
        });
        let count2 = count.clone();
        let sub2 = bus.subscribe("channel2", move |_| {
            sleep(Duration::from_millis(1000));
            *count2.lock().unwrap() += 1;
        });
        bus.notify("channel1", Bytes::from("data1"));
        bus.notify("channel1", Bytes::from("data2"));

        bus.notify("channel2", Bytes::from("data3"));
        bus.notify("channel2", Bytes::from("data4"));

        sleep(Duration::from_millis(500));
        assert_eq!(*count.lock().unwrap(), 0);
        sub2.cancel();
        sleep(Duration::from_millis(1000));
        assert_eq!(*count.lock().unwrap(), 2);
        sleep(Duration::from_millis(1000));
        assert_eq!(*count.lock().unwrap(), 3);
        sub1.cancel();
    }
    assert!(bus.num_channels() == 0);
}

#[test]
fn lazy_subscribe() {
    let bus = Bus::new(5);
    let count = Arc::new(Mutex::new(0));

    let sub1_activator = bus.lazy_subscribe("channel1");
    bus.notify("channel1", Bytes::from("data1"));

    let count1 = count.clone();
    let sub1 = sub1_activator.activate(move |msg| {
        assert_eq!(msg, Bytes::from("data1"));
        *count1.lock().unwrap() += 1;
    });
    sleep(Duration::from_millis(500));
    assert_eq!(*count.lock().unwrap(), 1);
    sub1.cancel();
}

#[test]
fn notify_exception() {
    let bus = Bus::new(5);
    let count = Arc::new(Mutex::new(0));

    let count1 = count.clone();
    let sub1 = bus.subscribe("channel1", move |_| {
        *count1.lock().unwrap() -= 1;
    });
    let count2 = count.clone();
    let sub2 = bus.subscribe("channel1", move |_| {
        *count2.lock().unwrap() += 1;
    });

    sub1.notify_others(Bytes::from("data1"));

    sleep(Duration::from_millis(500));
    assert_eq!(*count.lock().unwrap(), -1);
    sub1.cancel();
    sub2.cancel();
}

#[test]
fn bench_test() {
	let rand_bytes: Bytes = 
    Bytes::from((&mut SmallRng::from_entropy())
            .sample_iter(Standard)
            .take(MSG_SIZE - 2)
            .collect::<Vec<_>>(),
    );
	let bus = Bus::new(5);
	let count = Arc::new(Mutex::new(0));
	let count1 = count.clone();
	let start = Instant::now();
	let sub_activator = bus.lazy_subscribe("channel1");
	let mut i =0;
	while i!=NUM_MESSAGES{
		sleep(Duration::from_millis(1000));
		bus.notify("channel1",rand_bytes.clone());
		i=i+1;
	}

	let sub = sub_activator.activate(move |msg| {
		sleep(Duration::from_millis(1000));
        *count.lock().unwrap() += 1;
		println!("messsage is len:{:?} \n", msg.len());
    });

	sleep(Duration::from_millis(500));
    sub.cancel();
    let time_elapsed = start.elapsed().as_millis();
	
	//assert_eq!(*count1.lock().unwrap(), NUM_MESSAGES);
	println!("average connection time:{:?} len is {:?}\n", time_elapsed,*count1.lock().unwrap());
}
