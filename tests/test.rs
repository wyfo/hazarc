use std::{sync::Arc, thread};

use hazarc::{AtomicArc, AtomicOptionArc, borrow_list};

borrow_list!(TestList(1));

fn atomic_arc<T>(arc: Arc<T>) -> Arc<AtomicArc<T, TestList>> {
    Arc::new(arc.into())
}

fn atomic_option_arc<T>(arc: Option<Arc<T>>) -> Arc<AtomicOptionArc<T, TestList>> {
    Arc::new(arc.into())
}

#[test]
fn drop_atomic_arc_with_active_borrow() {
    let atomic_arc = atomic_arc(Arc::new(0));
    let borrow = atomic_arc.load();
    drop(atomic_arc);
    drop(borrow);
}

#[test]
fn drop_borrow_in_another_thread() {
    let arc = Arc::new(0);
    let atomic_arc = atomic_option_arc(Some(arc.clone()));
    let thread = thread::spawn({
        let atomic_arc = atomic_arc.clone();
        move || atomic_arc.load()
    });
    atomic_arc.store(None);
    let borrow = thread.join().unwrap();
    drop(borrow);
}
