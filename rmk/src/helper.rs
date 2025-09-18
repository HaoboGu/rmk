// #![allow(unused_macros)]

// /// Flatten nested tuple into a single tuple
// #[macro_export]
// macro_rules! flatten_tuple {
//     (($a:expr, ($($rest:tt)*))) => {
//         ($a, $crate::flatten_tuple!(($($rest)*)))
//     };
//     (($a:expr, $b:expr)) => {
//         ($a, $b)
//     };
//     (($a:expr,)) => {
//         ($a,)
//     };
//     ($other:expr) => {
//         $other
//     };
// }

// /// join_all macro: accept arbitrary number of futures, max using join4
// #[macro_export]
// macro_rules! join_all {
//     ($fut:expr) => {
//         $crate::flatten_tuple!(($fut.await,))
//     };
//     ($f1:expr, $f2:expr) => {
//         $crate::flatten_tuple!(embassy_futures::join::join($f1, $f2))
//     };
//     ($f1:expr, $f2:expr, $f3:expr) => {
//         $crate::flatten_tuple!(embassy_futures::join::join3($f1, $f2, $f3))
//     };
//     ($f1:expr, $f2:expr, $f3:expr, $f4:expr) => {
//         $crate::flatten_tuple!(embassy_futures::join::join4($f1, $f2, $f3, $f4))
//     };
//     ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $($rest:expr),+) => {{
//         let head = embassy_futures::join::join4($f1, $f2, $f3, $f4);
//         let tail = join_all!($($rest),+);
//         $crate::flatten_tuple!((head, tail))
//     }};
// }

// /// select_all macro: accept arbitrary number of futures, max using select4
// #[macro_export]
// macro_rules! select_all {
//     ($f:expr) => {
//         $f.await
//     };
//     ($f1:expr, $f2:expr) => {
//         embassy_futures::select::select($f1, $f2)
//     };
//     ($f1:expr, $f2:expr, $f3:expr) => {
//         embassy_futures::select::select3($f1, $f2, $f3)
//     };
//     ($f1:expr, $f2:expr, $f3:expr, $f4:expr) => {
//         embassy_futures::select::select4($f1, $f2, $f3, $f4)
//     };
//     ($f1:expr, $f2:expr, $f3:expr, $f4:expr, $($rest:expr),+) => {{
//         let head = embassy_futures::select::select4($f1, $f2, $f3, $f4);
//         let tail = select_all!($($rest),+);
//         $crate::flatten_tuple!((head, tail))
//     }};
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use embassy_futures::block_on;
//     use futures::FutureExt;

//     async fn f1() -> i32 {
//         1
//     }
//     async fn f2() -> u8 {
//         2
//     }
//     async fn f3() -> &'static str {
//         "3"
//     }
//     async fn f4() -> char {
//         '4'
//     }
//     async fn f5() -> i64 {
//         5
//     }

//     #[test]
//     fn test_join_all_flatten() {
//         let result = block_on(async { join_all!(f1(), f2(), f3(), f4(), f5()) });
//         let (a, b, c, d, e) = result;
//         println!("join_all_flatten = {:?}, {:?}, {:?}, {:?}, {:?}", a, b, c, d, e);
//     }

//     // #[test]
//     // fn test_select_all_flatten() {
//     //     use futures::future::Fuse;
//     //     // fuse all futures to safely use select
//     //     let futs: (Fuse<_>, Fuse<_>, Fuse<_>, Fuse<_>, Fuse<_>) =
//     //         (f1().fuse(), f2().fuse(), f3().fuse(), f4().fuse(), f5().fuse());

//     //     let result = block_on(async { select_all!(futs.0, futs.1, futs.2, futs.3, futs.4) });

//     //     // 因为 select 返回的是第一个完成的 future 的结果，然后递归 flatten
//     //     // 使用 pattern match 访问
//     //     let (r1, (r2, (r3, (r4, r5)))) = result;
//     //     println!("select_all_flatten = {:?}, {:?}, {:?}, {:?}, {:?}", r1, r2, r3, r4, r5);
//     // }
// }

/// Helper macro for joining all futures
#[macro_export]
macro_rules! join_all {
    ($first:expr, $second:expr, $($rest:expr),*) => {
        $crate::futures::future::join(
            $first,
            $crate::join_all!($second, $($rest),*)
        )
    };
    ($a:expr, $b:expr) => {
        $crate::futures::future::join($a, $b)
    };
    ($single:expr) => { $single };
}

#[macro_export]
macro_rules! with_feature {
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

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_futures::block_on;
    use futures::FutureExt;

    async fn f1() -> i32 {
        1
    }
    async fn f2() -> u8 {
        2
    }
    async fn f3() -> &'static str {
        "3"
    }
    async fn f4() -> char {
        '4'
    }
    async fn f5() -> i64 {
        5
    }

    #[test]
    fn test_join_all_flatten() {
        let result = block_on(async {
            join_all!(
                f1(),
                #[cfg(feature = "host")]
                f2(),
                f3(),
                f4(),
                f5()
            )
        });
        let x = result;
        // println!("join_all_flatten = {:?}, {:?}, {:?}, {:?}, {:?}", a, b, c, d, e);
    }
}
