#![allow(unused_macros)]

/// Helper macro for joining all futures
#[macro_export]
macro_rules! join_all {
    ($fut:expr) => {
        $fut
    };
    ($f1:expr, $f2:expr) => {
        $crate::embassy_futures::join::join($f1, $f2)
    };
    ($f1:expr, $f2:expr, $f3:expr) => {
        $crate::embassy_futures::join::join3($f1, $f2, $f3)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr) => {
        $crate::embassy_futures::join::join4($f1, $f2, $f3, $f4)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $($rest:expr),+) => {{
        let head = $crate::embassy_futures::join::join4($f1, $f2, $f3, $f4);
        let tail = $crate::join_all!($($rest),+);
        $crate::embassy_futures::join::join(head, tail)
    }};
}

#[macro_export]
macro_rules! with_feature {
    ($feature:literal, $future:expr, $t:ty) => {{
        #[cfg(feature = $feature)]
        {
            core::pin::pin!($future.fuse())
        }
        #[cfg(not(feature = $feature))]
        {
            core::future::pending::<$t>().fuse()
        }
    }};
    ($feature:literal, $future:expr) => {{
        #[cfg(feature = $feature)]
        {
            core::pin::pin!($future.fuse())
        }
        #[cfg(not(feature = $feature))]
        {
            core::future::pending::<()>().fuse()
        }
    }};
}
