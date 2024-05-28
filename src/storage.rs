use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug)]
pub struct TimeKeyValueStorage<Key, Value> {
    map: BTreeMap<DateTime<Utc>, (i64, Value)>, // i64 used directly for expiry time
    key_index: HashMap<Key, DateTime<Utc>>,
}

impl<Key, Value> TimeKeyValueStorage<Key, Value>
where
    Key: std::cmp::Eq + std::hash::Hash + Clone,
{
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            key_index: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: Key, value: Value, expiry: i64) {
        let now = Utc::now();
        self.map.insert(now, (expiry, value));
        self.key_index.insert(key, now);
    }

    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.key_index.get(key).and_then(|timestamp| {
            self.map.get(timestamp).and_then(|(expiry, value)| {
                if *expiry == i64::MAX
                    || timestamp.timestamp_millis() + expiry >= Utc::now().timestamp_millis()
                {
                    Some(value)
                } else {
                    None
                }
            })
        })
    }

    pub fn get_last_modified(&self, key: &Key) -> Option<&DateTime<Utc>> {
        self.key_index.get(key)
    }

    pub fn update(&mut self, key: Key, value: Value, expiry: i64) {
        if let Some(&timestamp) = self.key_index.get(&key) {
            self.map.remove(&timestamp);
        }
        self.insert(key, value, expiry);
    }

    pub fn get_by_time(&self, timestamp: &DateTime<Utc>) -> Option<&(i64, Value)> {
        self.map.get(timestamp)
    }
}
