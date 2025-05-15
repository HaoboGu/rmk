mod common;
pub(crate) use crate::common::*;

mod keyboard_test {

    use super::*;

    use embassy_futures::block_on;
    use log::{debug, info};
    use rusty_fork::rusty_fork_test;


    #[test]
    #[ignore]
    pub fn test_example() {
        let main = async {};
        block_on(main);
    }

    /// demo for test need fork
    rusty_fork_test! {

    #[test]
    #[ignore]
    fn test_keyboard_example() {

        let main = async {
            debug!("hello")
        };
        block_on(main);
    }

    }
}
