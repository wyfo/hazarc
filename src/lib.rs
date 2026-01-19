#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]
extern crate alloc;

use alloc::sync::Arc;

pub mod borrow_list;
pub mod cache;
pub mod generic;

#[cfg(feature = "default-borrow-list")]
borrow_list!(pub DefaultBorrowList(8));

#[cfg(feature = "default-borrow-list")]
pub type AtomicArc<T, L = DefaultBorrowList> = generic::AtomicArcPtr<Arc<T>, L>;
#[cfg(not(feature = "default-borrow-list"))]
pub type AtomicArc<T, L> = generic::AtomicArcPtr<Arc<T>, L>;
#[cfg(feature = "default-borrow-list")]
pub type AtomicOptionArc<T, L = DefaultBorrowList> = generic::AtomicArcPtr<Option<Arc<T>>, L>;
#[cfg(not(feature = "default-borrow-list"))]
pub type AtomicOptionArc<T, L> = generic::AtomicArcPtr<Option<Arc<T>>, L>;
pub type ArcBorrow<T> = generic::ArcPtrBorrow<Arc<T>>;

const NULL: *mut () = core::ptr::null_mut();

#[macro_export]
macro_rules! borrow_list {
    ($vis:vis $name:ident($borrow_count:expr)) => {
        $vis struct $name;
        unsafe impl $crate::borrow_list::StaticBorrowList for $name {
            #[inline(always)]
            fn static_list() -> &'static $crate::borrow_list::BorrowList {
                static LIST: $crate::borrow_list::BorrowList = $crate::borrow_list::BorrowList::new();
                &LIST
            }
            #[inline(always)]
            fn thread_local_node() -> $crate::borrow_list::BorrowNodeRef {
                extern crate std;
                std::thread_local! {
                    static LOCAL: std::cell::Cell<std::option::Option<$crate::borrow_list::BorrowNodeRef>> = const { std::cell::Cell::new(None) };
                }
                #[cold]
                #[inline(never)]
                fn new_node() -> $crate::borrow_list::BorrowNodeRef {
                    struct NodeGuard;
                    impl Drop for NodeGuard {
                        fn drop(&mut self) {
                            if let Some(node) = LOCAL.take() {
                                unsafe { <$name as  $crate::borrow_list::StaticBorrowList>::static_list().remove_node(node) };
                            }
                        }
                    }
                    std::thread_local! {
                        static GUARD: NodeGuard = const { NodeGuard };
                    }
                    let node = <$name as  $crate::borrow_list::StaticBorrowList>::static_list().insert_node($borrow_count);
                    LOCAL.set(Some(node));
                    GUARD.with(|_| ());
                    node
                }
                LOCAL.get().unwrap_or_else(new_node)
            }
        }
    };
}
