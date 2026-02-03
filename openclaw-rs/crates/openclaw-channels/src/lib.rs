//! Messaging channel abstractions for OpenClaw.
//!
//! This crate provides the core traits and types for messaging channels,
//! along with routing and delivery mechanisms.

pub mod error;
pub mod traits;
pub mod routing;
pub mod delivery;
pub mod attachment;
pub mod registry;

#[cfg(feature = "telegram")]
pub mod telegram;

#[cfg(feature = "discord")]
pub mod discord;

pub use error::ChannelError;
pub use traits::{Channel, ChannelReceiver, ChannelSender, ChannelLifecycle};
pub use routing::{Router, RouteMatch, RouteRule};
pub use delivery::{DeliveryQueue, DeliveryStatus, DeliveryResult};
pub use attachment::{Attachment, AttachmentType};
pub use registry::{ChannelRegistry, RegisteredChannel};

/// Result type for channel operations.
pub type Result<T> = std::result::Result<T, ChannelError>;
