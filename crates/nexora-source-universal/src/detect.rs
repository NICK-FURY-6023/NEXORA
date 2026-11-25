/// Maps a URL or URI to the adapter that owns it.
///
/// Ordered literal matches, no network and no allocations beyond the
/// lowercased input. Extend this list when a new adapter lands.
const PATTERNS: &[(&str, &str)] = &[
    ("archive.org/details/", "internet-archive"),
    ("bandcamp.com/track/", "bandcamp-public"),
    ("bandcamp.com/album/", "bandcamp-public"),
    ("soundcloud.com/", "soundcloud-public"),
    ("youtube.com/watch", "youtube"),
    ("youtu.be/", "youtube"),
    ("open.spotify.com/track/", "spotify"),
    ("open.spotify.com/album/", "spotify"),
    ("spotify:track:", "spotify"),
];

pub fn detect(input: &str) -> Option<&'static str> {
    let input = input.to_ascii_lowercase();
    PATTERNS
        .iter()
        .find_map(|(needle, id)| input.contains(needle).then_some(*id))
}
