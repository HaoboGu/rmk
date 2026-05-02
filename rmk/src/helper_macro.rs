#![allow(unused_macros)]

/// Helper macro for joining all futures
#[macro_export]
macro_rules! join_all {
    ($fut:expr$(,)?) => {
        $fut
    };
    ($f1:expr, $f2:expr$(,)?) => {
        $crate::embassy_futures::join::join($f1, $f2)
    };
    ($f1:expr, $f2:expr, $f3:expr$(,)?) => {
        $crate::embassy_futures::join::join3($f1, $f2, $f3)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr$(,)?) => {
        $crate::embassy_futures::join::join4($f1, $f2, $f3, $f4)
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr$(,)?) => {
        $crate::embassy_futures::join::join5($f1, $f2, $f3, $f4, $f5)
    };
    // 6+: chunk into groups of 5, then recurse on the chunk list so
    // the outer combinator widens (join, join3, join4, join5) before
    // nesting again.
    ($($all:expr),+ $(,)?) => {
        $crate::__join_all_chunked!(chunks=[] $($all,)+)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __join_all_chunked {
    // Peel a full chunk of 5 when more inputs remain.
    (chunks=[$($chunks:expr,)*] $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr, $($rest:tt)+) => {
        $crate::__join_all_chunked!(
            chunks=[$($chunks,)* $crate::embassy_futures::join::join5($f1, $f2, $f3, $f4, $f5),]
            $($rest)+
        )
    };
    // Final partial chunk (1..=5 remaining): re-invoke join_all on the chunk list,
    // which widens the outer combinator and recurses if there are more than 5 chunks.
    (chunks=[$($chunks:expr,)*] $f1:expr $(,)?) => {
        $crate::join_all!($($chunks,)* $f1)
    };
    (chunks=[$($chunks:expr,)*] $f1:expr, $f2:expr $(,)?) => {
        $crate::join_all!($($chunks,)* $crate::embassy_futures::join::join($f1, $f2))
    };
    (chunks=[$($chunks:expr,)*] $f1:expr, $f2:expr, $f3:expr $(,)?) => {
        $crate::join_all!($($chunks,)* $crate::embassy_futures::join::join3($f1, $f2, $f3))
    };
    (chunks=[$($chunks:expr,)*] $f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        $crate::join_all!($($chunks,)* $crate::embassy_futures::join::join4($f1, $f2, $f3, $f4))
    };
    (chunks=[$($chunks:expr,)*] $f1:expr, $f2:expr, $f3:expr, $f4:expr, $f5:expr $(,)?) => {
        $crate::join_all!($($chunks,)* $crate::embassy_futures::join::join5($f1, $f2, $f3, $f4, $f5))
    };
}

#[macro_export]
macro_rules! with_feature {
    ($feature:literal, $future:expr, $t:ty$(,)?) => {{
        #[cfg(feature = $feature)]
        {
            core::pin::pin!($future.fuse())
        }
        #[cfg(not(feature = $feature))]
        {
            core::future::pending::<$t>().fuse()
        }
    }};
    ($feature:literal, $future:expr$(,)?) => {{
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

/// Wrapper for select_biased! with feature-gated arms
///
/// Usage:
/// ```ignore
/// select_biased_with_feature! {
///     pattern = future => handler,
///     with_feature("feature"): pattern = future => handler,
/// }
/// ```
#[macro_export]
macro_rules! select_biased_with_feature {
    ($($input:tt)*) => {
        $crate::__select_biased_with_feature_impl!([] [] $($input)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __select_biased_with_feature_impl {
    // Collect conditional arm
    ([$($n:tt)*] [$($c:tt)*] with_feature($f:literal): $p:pat = $fut:expr => $h:expr, $($rest:tt)*) => {
        $crate::__select_biased_with_feature_impl!([$($n)*] [$($c)* {$f: $p = $fut => $h,}] $($rest)*)
    };
    ([$($n:tt)*] [$($c:tt)*] with_feature($f:literal): $p:pat = $fut:expr => $h:expr $(,)?) => {
        $crate::__select_biased_with_feature_impl!([$($n)*] [$($c)* {$f: $p = $fut => $h,}])
    };

    // Collect normal arm
    ([$($n:tt)*] [$($c:tt)*] $p:pat = $fut:expr => $h:expr, $($rest:tt)*) => {
        $crate::__select_biased_with_feature_impl!([$($n)* $p = $fut => $h,] [$($c)*] $($rest)*)
    };
    ([$($n:tt)*] [$($c:tt)*] $p:pat = $fut:expr => $h:expr $(,)?) => {
        $crate::__select_biased_with_feature_impl!([$($n)* $p = $fut => $h,] [$($c)*])
    };

    // Done: no conditional arms
    ([$($n:tt)*] []) => {
        $crate::futures::select_biased! { $($n)* }
    };

    // Done: has conditional arms - generate nested cfg
    ([$($n:tt)*] [{$f:literal: $($arm:tt)*} $($rest:tt)*]) => {{
        #[cfg(feature = $f)]
        { $crate::__select_biased_with_feature_gen!([$($n)* $($arm)*] [$($rest)*]) }
        #[cfg(not(feature = $f))]
        { $crate::__select_biased_with_feature_impl!([$($n)*] [$($rest)*]) }
    }};
}

// Generate final select_biased with collected arms
#[doc(hidden)]
#[macro_export]
macro_rules! __select_biased_with_feature_gen {
    ([$($n:tt)*] [$($c:tt)*]) => {
        $crate::__select_biased_with_feature_expand!([$($n)*] [$($c)*])
    };
}

// Expand to select_biased with cfg checks for remaining conditional arms
#[doc(hidden)]
#[macro_export]
macro_rules! __select_biased_with_feature_expand {
    ([$($all:tt)*] []) => {
        $crate::futures::select_biased! { $($all)* }
    };
    ([$($collected:tt)*] [{$f:literal: $($arm:tt)*} $($rest:tt)*]) => {{
        #[cfg(feature = $f)]
        { $crate::__select_biased_with_feature_expand!([$($collected)* $($arm)*] [$($rest)*]) }
        #[cfg(not(feature = $f))]
        { $crate::__select_biased_with_feature_expand!([$($collected)*] [$($rest)*]) }
    }};
}
