//! The Lavalink v4 session context: one session, many guild players.
//!
//! A Lavalink v4 session is the WebSocket connection context. Inside it every
//! Discord guild has its own player. NEXORA keeps the queue in the native
//! session manager and models here what the v4 surface exposes: the players
//! and their state.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use nexora_protocol::{
    V4EventMessage, V4Filters, V4Frame, V4Player, V4PlayerState, V4PlayerUpdateFrame, V4Track,
    V4VoiceState,
};
use tokio::sync::{RwLock, broadcast};

/// The alphabet the reference draws session ids from.
const SESSION_ID_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// The length of a reference session id.
const SESSION_ID_LENGTH: usize = 16;

/// The reference's default resume timeout, in seconds.
pub const DEFAULT_RESUME_TIMEOUT: u64 = 60;

/// The reference's `playerUpdateInterval` default, in seconds.
pub const PLAYER_UPDATE_INTERVAL_SECONDS: u64 = 5;

/// One player inside a Lavalink v4 session.
#[derive(Debug, Clone)]
pub struct V4PlayerRecord {
    pub guild_id: String,
    pub track: Option<V4Track>,
    pub volume: i64,
    pub paused: bool,
    pub position_ms: u64,
    /// The requested end time, in milliseconds.
    ///
    /// The reference models `endTime` as a request-only field: it bounds the
    /// track's playback but is not part of the player object the endpoints
    /// return, so it is stored here and never serialised.
    pub end_time_ms: Option<u64>,
    pub filters: V4Filters,
    pub voice: V4VoiceState,
    pub user_data: serde_json::Value,
}

impl V4PlayerRecord {
    fn new(guild_id: impl Into<String>) -> Self {
        Self {
            guild_id: guild_id.into(),
            track: None,
            volume: 100,
            paused: false,
            position_ms: 0,
            end_time_ms: None,
            filters: V4Filters::default(),
            voice: V4VoiceState::default(),
            user_data: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// The state block the reference sends in a `playerUpdate` frame.
    pub fn state(&self) -> V4PlayerState {
        V4PlayerState {
            time: now_ms(),
            position: self.position_ms as i64,
            connected: self.voice.is_complete(),
            ping: -1,
        }
    }

    /// The player object the reference returns from the player endpoints.
    pub fn to_player(&self) -> V4Player {
        V4Player {
            guild_id: self.guild_id.clone(),
            track: self.track.clone(),
            volume: self.volume,
            paused: self.paused,
            state: self.state(),
            voice: self.voice.clone(),
            filters: self.filters.clone(),
        }
    }
}

/// A Lavalink v4 session context.
#[derive(Debug, Clone)]
pub struct V4SessionContext {
    pub session_id: String,
    pub players: HashMap<String, V4PlayerRecord>,
    pub resuming: bool,
    pub resume_timeout: u64,
}

/// Holds the Lavalink v4 sessions and their players.
#[derive(Debug)]
pub struct V4SessionManager {
    sessions: RwLock<HashMap<String, V4SessionContext>>,
    events: broadcast::Sender<V4Frame>,
    counter: AtomicU64,
}

impl Default for V4SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl V4SessionManager {
    pub fn new() -> Self {
        let (events, _receiver) = broadcast::channel(256);
        Self {
            sessions: RwLock::new(HashMap::new()),
            events,
            counter: AtomicU64::new(0),
        }
    }

    /// Subscribes to the session's emitted frames.
    pub fn subscribe(&self) -> broadcast::Receiver<V4Frame> {
        self.events.subscribe()
    }

    /// Emits one event to every subscriber.
    pub fn emit(&self, event: V4EventMessage) {
        let _ = self.events.send(V4Frame::Event(Box::new(event)));
    }

    /// Emits one `playerUpdate` frame to every subscriber.
    pub fn emit_player_update(&self, frame: V4PlayerUpdateFrame) {
        let _ = self.events.send(V4Frame::PlayerUpdate(Box::new(frame)));
    }

    /// Creates a session and returns its id.
    ///
    /// The reference draws sixteen characters uniformly from `a-z0-9` and
    /// retries until the id is unused.
    pub async fn create_session(&self) -> String {
        let mut sessions = self.sessions.write().await;
        loop {
            let sequence = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
            let id = random_session_id(sequence, seed());
            if !sessions.contains_key(&id) {
                sessions.insert(
                    id.clone(),
                    V4SessionContext {
                        session_id: id.clone(),
                        players: HashMap::new(),
                        resuming: false,
                        resume_timeout: DEFAULT_RESUME_TIMEOUT,
                    },
                );
                return id;
            }
        }
    }

    /// Returns the session, or `None` when it does not exist.
    pub async fn session(&self, session_id: &str) -> Option<V4SessionContext> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Returns the session's players, or `None` when the session does not exist.
    pub async fn players(&self, session_id: &str) -> Option<Vec<V4PlayerRecord>> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)?;
        let mut players: Vec<V4PlayerRecord> = session.players.values().cloned().collect();
        players.sort_by(|left, right| left.guild_id.cmp(&right.guild_id));
        Some(players)
    }

    /// Returns one player, or `None` when the session or the player is absent.
    pub async fn player(&self, session_id: &str, guild_id: &str) -> Option<V4PlayerRecord> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id)?.players.get(guild_id).cloned()
    }

    /// Applies a change to one player, creating it when it does not exist.
    ///
    /// Returns the player after the change, or `None` when the session does
    /// not exist.
    pub async fn update_player<F>(
        &self,
        session_id: &str,
        guild_id: &str,
        apply: F,
    ) -> Option<V4PlayerRecord>
    where
        F: FnOnce(&mut V4PlayerRecord),
    {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)?;
        let player = session
            .players
            .entry(guild_id.to_owned())
            .or_insert_with(|| V4PlayerRecord::new(guild_id));
        apply(player);
        Some(player.clone())
    }

    /// Removes a player. Returns `true` when one was removed.
    pub async fn remove_player(&self, session_id: &str, guild_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        sessions
            .get_mut(session_id)
            .is_some_and(|session| session.players.remove(guild_id).is_some())
    }

    /// Sets the session's resume behaviour.
    ///
    /// Returns the session's resume state, or `None` when it does not exist.
    pub async fn set_resuming(
        &self,
        session_id: &str,
        resuming: Option<bool>,
        timeout: Option<u64>,
    ) -> Option<(bool, u64)> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)?;
        if let Some(resuming) = resuming {
            session.resuming = resuming;
        }
        if let Some(timeout) = timeout {
            session.resume_timeout = timeout;
        }
        Some((session.resuming, session.resume_timeout))
    }

    /// Removes a session and every player in it.
    pub async fn remove_session(&self, session_id: &str) -> bool {
        self.sessions.write().await.remove(session_id).is_some()
    }
}

/// The current Unix time in milliseconds.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

/// A seed for the session-id generator.
fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0)
}

/// Draws a session id from the reference's alphabet.
///
/// A xorshift generator seeded from the clock and the session counter: the
/// reference uses a uniform random draw, and this only has to be unique.
fn random_session_id(counter: u64, seed: u64) -> String {
    let mut state = seed ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    if state == 0 {
        state = 0x9E37_79B9_7F4A_7C15;
    }
    let mut id = String::with_capacity(SESSION_ID_LENGTH);
    for _ in 0..SESSION_ID_LENGTH {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        id.push(SESSION_ID_ALPHABET[(state % SESSION_ID_ALPHABET.len() as u64) as usize] as char);
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexora_protocol::{V4Track, V4TrackInfo};

    fn track(title: &str) -> V4Track {
        V4Track {
            encoded: "abc".into(),
            info: V4TrackInfo {
                identifier: "tone.wav".into(),
                is_seekable: true,
                author: "artist".into(),
                length: 100,
                is_stream: false,
                position: 0,
                title: title.into(),
                uri: None,
                artwork_url: None,
                isrc: None,
                source_name: "local".into(),
            },
            plugin_info: serde_json::json!({}),
            user_data: serde_json::json!({}),
        }
    }

    #[tokio::test]
    async fn session_ids_are_sixteen_lowercase_alphanumerics() {
        let manager = V4SessionManager::new();
        let id = manager.create_session().await;
        assert_eq!(id.len(), 16);
        assert!(
            id.chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        );
        let second = manager.create_session().await;
        assert_ne!(id, second);
    }

    #[tokio::test]
    async fn a_player_is_created_on_first_update() {
        let manager = V4SessionManager::new();
        let session = manager.create_session().await;
        assert!(manager.player(&session, "1234").await.is_none());

        let player = manager
            .update_player(&session, "1234", |player| {
                player.track = Some(track("Tone"));
                player.volume = 80;
            })
            .await
            .expect("player");
        assert_eq!(player.guild_id, "1234");
        assert_eq!(player.volume, 80);
        assert!(player.track.is_some());
        assert_eq!(manager.players(&session).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn players_are_listed_in_guild_id_order() {
        let manager = V4SessionManager::new();
        let session = manager.create_session().await;
        manager.update_player(&session, "9999", |_| {}).await;
        manager.update_player(&session, "1111", |_| {}).await;
        let players = manager.players(&session).await.unwrap();
        assert_eq!(players[0].guild_id, "1111");
        assert_eq!(players[1].guild_id, "9999");
    }

    #[tokio::test]
    async fn removing_a_player_removes_only_that_player() {
        let manager = V4SessionManager::new();
        let session = manager.create_session().await;
        manager.update_player(&session, "1111", |_| {}).await;
        manager.update_player(&session, "2222", |_| {}).await;
        assert!(manager.remove_player(&session, "1111").await);
        assert!(!manager.remove_player(&session, "1111").await);
        assert_eq!(manager.players(&session).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn resuming_and_timeout_are_kept_per_session() {
        let manager = V4SessionManager::new();
        let session = manager.create_session().await;
        let (resuming, timeout) = manager
            .set_resuming(&session, Some(true), Some(120))
            .await
            .unwrap();
        assert!(resuming);
        assert_eq!(timeout, 120);
        let (resuming, timeout) = manager.set_resuming(&session, None, None).await.unwrap();
        assert!(resuming);
        assert_eq!(timeout, 120);
    }

    #[tokio::test]
    async fn a_missing_session_has_no_players() {
        let manager = V4SessionManager::new();
        assert!(manager.players("nope").await.is_none());
        assert!(manager.update_player("nope", "1", |_| {}).await.is_none());
    }

    #[tokio::test]
    async fn events_reach_subscribers() {
        let manager = V4SessionManager::new();
        let mut receiver = manager.subscribe();
        manager.emit(V4EventMessage::WebSocketClosed {
            guild_id: "1234".into(),
            code: 1000,
            reason: "bye".into(),
            by_remote: false,
        });
        let frame = receiver.recv().await.unwrap();
        let V4Frame::Event(event) = frame else {
            panic!("an emitted event arrives as an event frame");
        };
        assert_eq!(event.type_name(), "WebSocketClosedEvent");
    }

    #[tokio::test]
    async fn a_player_update_frame_is_its_own_op() {
        let manager = V4SessionManager::new();
        let mut receiver = manager.subscribe();
        manager.emit_player_update(V4PlayerUpdateFrame {
            state: V4PlayerState {
                time: 1,
                position: 2,
                connected: false,
                ping: -1,
            },
            guild_id: "1234".into(),
        });
        let frame = receiver.recv().await.unwrap();
        let V4Frame::PlayerUpdate(update) = frame else {
            panic!("a player update arrives as a playerUpdate frame");
        };
        assert_eq!(update.guild_id, "1234");
        assert_eq!(update.state.position, 2);
    }
}
