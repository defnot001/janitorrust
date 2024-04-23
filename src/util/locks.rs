use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, OnceLock, Weak,
};

use dashmap::DashMap;
use serenity::all::UserId;
use tokio::sync::{Mutex, MutexGuard, OwnedMutexGuard};

pub struct SelfRemoving {
    id: u64,
    user_id: UserId,
}

struct LockValue {
    id: u64,
    weak: Weak<Mutex<SelfRemoving>>,
}

impl LockValue {
    fn new(user_id: UserId) -> (Self, Arc<Mutex<SelfRemoving>>) {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let new_arc = Arc::new(Mutex::new(SelfRemoving { user_id, id }));
        let lock_value = LockValue {
            id,
            weak: Arc::downgrade(&new_arc),
        };

        (lock_value, new_arc)
    }
}

impl Drop for SelfRemoving {
    fn drop(&mut self) {
        locks().remove_if(&self.user_id, |_, value| value.id == self.id);
    }
}

type LocksMap = DashMap<UserId, LockValue>;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static LOCKS: OnceLock<LocksMap> = OnceLock::new();

fn locks() -> &'static LocksMap {
    LOCKS.get_or_init(DashMap::new)
}

#[must_use]
pub async fn lock_user_id(user_id: UserId) -> OwnedMutexGuard<SelfRemoving> {
    match locks().entry(user_id) {
        dashmap::mapref::entry::Entry::Occupied(mut entry) => {
            if let Some(upgraded) = entry.get().weak.upgrade() {
                upgraded.lock_owned().await
            } else {
                let (lock_value, new_arc) = LockValue::new(user_id);
                entry.insert(lock_value);
                new_arc.lock_owned().await
            }
        }
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            let (lock_value, new_arc) = LockValue::new(user_id);
            entry.insert(lock_value);
            new_arc.lock_owned().await
        }
    }
}
