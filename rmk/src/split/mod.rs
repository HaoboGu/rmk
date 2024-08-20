use master::SplitMessage;
use postcard::experimental::max_size::MaxSize;

pub(crate) mod driver;
pub mod master;
pub mod slave;

pub const SPLIT_MESSAGE_MAX_SIZE: usize = SplitMessage::POSTCARD_MAX_SIZE + 4;
