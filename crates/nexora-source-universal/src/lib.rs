mod detect;

pub mod apple;
pub mod archive;
pub mod bandcamp;
pub mod deezer;
pub mod oauth;
pub mod oembed;
pub mod simple;
pub mod soundcloud;
pub mod spotify;
pub mod tidal;
pub mod transport;
pub mod youtube;

pub const PLATFORM_IDS: [&str; 8] = [
    "internet-archive",
    "bandcamp-public",
    "soundcloud-public",
    "youtube",
    "spotify",
    "deezer",
    "apple-music",
    "tidal",
];

pub use detect::detect;
