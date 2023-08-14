use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
/// Interface for receiving messages from the network. Created by calling
/// `Builder::add_subscriber()` during network setup.
///

#[derive(Clone)]
pub struct LocalBusSubscriber<Topic, Content> {
    pub inbox: Receiver<(Topic, Content)>,
}

impl<Topic, Content> LocalBusSubscriber<Topic, Content> {
    /// Consumes all pending messages in the subscriber's inbox.
    pub fn fetch(&self) -> Vec<(Topic, Content)> {
        // TODO: Instead of a Vec, use some kind of iterator
        let mut messages = vec![];
        while let Ok(message) = self.inbox.try_recv() {
            messages.push(message);
        }
        messages
    }
}

/// Interface for sending messages to the network. To add more publishers, just
/// clone this object and distribute the clones to your clients.
pub struct InnerLocalBusPublisher<Topic: Hash + Eq + Clone, Content: Clone> {
    subscribers: RwLock<HashMap<Topic, Vec<Sender<(Topic, Content)>>>>,
}

#[derive(Clone)]
pub struct LocalBusPublisher<Topic: Hash + Eq + Clone, Content: Clone>(
    Arc<InnerLocalBusPublisher<Topic, Content>>,
);

impl<Topic: Hash + Eq + Clone, Content: Clone> Deref for LocalBusPublisher<Topic, Content> {
    type Target = Arc<InnerLocalBusPublisher<Topic, Content>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Topic: Hash + Eq + Clone, Content: Clone> LocalBusPublisher<Topic, Content> {
    /// Called to initialize a network.
    pub fn new() -> LocalBusPublisher<Topic, Content> {
        LocalBusPublisher {
            0: Arc::new(InnerLocalBusPublisher {
                subscribers: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Sends a message to the network. All topic filtering is done in the
    /// calling thread.
    pub fn publish(&self, topic: Topic, content: Content) {
        let binding = self.subscribers.read();
        let outbox = match binding.get(&topic) {
            Some(o) => o,
            None => return,
        };

        for subscriber in outbox {
            subscriber
                .send((topic.clone(), content.clone()))
                .unwrap_or(());
        }
    }

    fn add_subscriber(&self, topic: Topic, tx: Sender<(Topic, Content)>) {
        let mut binding = self.subscribers.write();
        let subscriber_list = binding.entry(topic);
        subscriber_list
            .or_insert_with(|| Vec::new())
            .push(tx.clone());
    }
}

/// Helper for building networks. Call `build()` to complete initialization.
pub struct InnerLocalBusBuilder<Topic: Hash + Eq + Clone, Content: Clone> {
    publisher: LocalBusPublisher<Topic, Content>,
}

#[derive(Clone)]
pub struct LocalBusBuilder<Topic: Hash + Eq + Clone, Content: Clone>(
    Arc<InnerLocalBusBuilder<Topic, Content>>,
);

impl<Topic: Hash + Eq + Clone, Content: Clone> Deref for LocalBusBuilder<Topic, Content> {
    type Target = Arc<InnerLocalBusBuilder<Topic, Content>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Topic: Hash + Eq + Clone, Content: Clone> LocalBusBuilder<Topic, Content> {
    /// Adds a subscriber to the network, with a complete list of the Topics it
    /// expects to receive. This list cannot be modified later.
    pub fn add_subscriber(&self, topics: &[Topic]) -> LocalBusSubscriber<Topic, Content> {
        let (tx, rx) = bounded(1024);
        for topic in topics {
            let topic = topic.clone();
            // let subscriber_list = self.publisher.subscribers.entry(topic);
            // subscriber_list
            //     .or_insert_with(|| Vec::new())
            //     .push(tx.clone());
            self.publisher.add_subscriber(topic, tx.clone());
        }

        LocalBusSubscriber { inbox: rx }
    }

    /// Finishes network setup. No more subscribers can be added after this.
    pub fn publisher(&self) -> LocalBusPublisher<Topic, Content> {
        self.publisher.clone()
    }

    pub fn new() -> LocalBusBuilder<Topic, Content> {
        LocalBusBuilder {
            0: Arc::new(InnerLocalBusBuilder {
                publisher: LocalBusPublisher::new(),
            }),
        }
    }
}
