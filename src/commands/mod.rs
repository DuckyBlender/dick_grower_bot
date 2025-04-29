pub mod dotd;
pub mod global;
pub mod grow;
pub mod help;
pub mod pvp;
pub mod stats;
pub mod top;

// Re-export all command handlers
pub use dotd::handle_dotd_command;
pub use global::handle_global_command;
pub use grow::handle_grow_command;
pub use help::handle_help_command;
pub use pvp::{PvpChallenge, handle_pvp_accept, handle_pvp_command};
pub use stats::handle_stats_command;
pub use top::handle_top_command;
pub use crate::utils::escape_markdown;
