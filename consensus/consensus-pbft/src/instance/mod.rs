use utils::general::MILLI_UNITS_PER_SEC;

pub mod bft_instance;
pub mod bft_instance_index;
pub mod bft_vc_instance;

pub const PBFT_VCINSTANCE_TIMEOUT: i64 = 60 * MILLI_UNITS_PER_SEC;
pub const PBFT_INSTANCE_TIMEOUT: i64 = 30 * MILLI_UNITS_PER_SEC;
pub const PBFT_COMMIT_SEND_INTERVAL: i64 = 15 * MILLI_UNITS_PER_SEC;
pub const PBFT_NEWVIEW_SEND_INTERVAL: i64 = 15 * MILLI_UNITS_PER_SEC;
