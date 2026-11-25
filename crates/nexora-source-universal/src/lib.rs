mod detect;

pub mod archive;
pub mod bandcamp;
pub mod oauth;
pub mod oembed;
pub mod soundcloud;
pub mod spotify;
pub mod youtube;

pub use detect::detect;
