//! The Nexora nexora player object, its state, and the player-update body.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::filters::NexoraFilters;
use super::track::NexoraTrack;

/// A field the reference models as `Omissible<T?>`: absent, explicitly null,
/// or present with a value.
///
/// The three states are distinguishable: `None` is absent, `Some(None)` is an
/// explicit `null`, `Some(Some(value))` carries a value.
pub type NexoraNullable<T> = Option<Option<T>>;

/// Deserialises a field that may be absent or explicitly null.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// The Nexora nexora player state block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraPlayerState {
    /// Epoch milliseconds at the moment the update was emitted.
    pub time: i64,
    /// The playing track's position in milliseconds, or `0`.
    pub position: i64,
    /// Whether the voice gateway connection is open.
    pub connected: bool,
    /// The gateway ping in milliseconds, or `-1` when unavailable.
    pub ping: i64,
}

impl Default for NexoraPlayerState {
    fn default() -> Self {
        Self {
            time: 0,
            position: 0,
            connected: false,
            ping: -1,
        }
    }
}

/// The Nexora nexora voice state.
///
/// The shipped reference carries the gateway token, the endpoint, the voice
/// session id and the channel id — there is no `serverId`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexoraVoiceState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, rename = "channelId", skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
}

impl NexoraVoiceState {
    /// Whether the state carries every field the reference requires.
    pub fn is_complete(&self) -> bool {
        self.token.as_deref().is_some_and(|value| !value.is_empty())
            && self
                .endpoint
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            && self
                .session_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            && self
                .channel_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
    }
}

/// The Nexora nexora player object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraPlayer {
    #[serde(rename = "guildId")]
    pub guild_id: String,
    pub track: Option<NexoraTrack>,
    pub volume: i64,
    pub paused: bool,
    pub state: NexoraPlayerState,
    pub voice: NexoraVoiceState,
    pub filters: NexoraFilters,
}

/// The track block of a player update.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NexoraPlayerUpdateTrack {
    #[serde(
        default,
        deserialize_with = "deserialize_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub encoded: NexoraNullable<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, rename = "userData", skip_serializing_if = "Option::is_none")]
    pub user_data: Option<Value>,
}

/// The request body of `PATCH /nexora/sessions/{sessionId}/players/{guildId}`.
///
/// `encodedTrack` and `endTime` are nullable in the reference: an explicit
/// `null` is meaningful and differs from an absent field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NexoraPlayerUpdate {
    #[serde(
        default,
        rename = "encodedTrack",
        deserialize_with = "deserialize_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub encoded_track: NexoraNullable<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<NexoraPlayerUpdateTrack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u64>,
    #[serde(
        default,
        rename = "endTime",
        deserialize_with = "deserialize_some",
        skip_serializing_if = "Option::is_none"
    )]
    pub end_time: NexoraNullable<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<NexoraFilters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<NexoraVoiceState>,
}

/// The `GET /nexora/sessions/{sessionId}/players` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexoraPlayers {
    pub players: Vec<NexoraPlayer>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn player_serialises_the_reference_field_names() {
        let player = NexoraPlayer {
            guild_id: "1234".into(),
            track: None,
            volume: 100,
            paused: false,
            state: NexoraPlayerState::default(),
            voice: NexoraVoiceState::default(),
            filters: NexoraFilters::default(),
        };
        let value = serde_json::to_value(player).unwrap();
        assert_eq!(value["guildId"], "1234");
        assert_eq!(value["volume"], 100);
        assert_eq!(value["paused"], false);
        assert_eq!(value["state"]["ping"], -1);
        assert_eq!(value["state"]["connected"], false);
        assert!(value["track"].is_null());
        assert!(value.get("guild_id").is_none());
    }

    #[test]
    fn voice_state_carries_the_four_reference_fields() {
        let voice = NexoraVoiceState {
            token: Some("t".into()),
            endpoint: Some("e".into()),
            session_id: Some("s".into()),
            channel_id: Some("c".into()),
        };
        let value = serde_json::to_value(&voice).unwrap();
        assert_eq!(value["token"], "t");
        assert_eq!(value["endpoint"], "e");
        assert_eq!(value["sessionId"], "s");
        assert_eq!(value["channelId"], "c");
        assert!(voice.is_complete());
    }

    #[test]
    fn an_explicit_null_encoded_track_differs_from_an_absent_one() {
        let update: NexoraPlayerUpdate =
            serde_json::from_value(json!({ "encodedTrack": null })).unwrap();
        assert_eq!(update.encoded_track, Some(None));

        let absent: NexoraPlayerUpdate = serde_json::from_value(json!({})).unwrap();
        assert_eq!(absent.encoded_track, None);
    }

    #[test]
    fn an_explicit_null_end_time_differs_from_an_absent_one() {
        let update: NexoraPlayerUpdate =
            serde_json::from_value(json!({ "endTime": null })).unwrap();
        assert_eq!(update.end_time, Some(None));

        let absent: NexoraPlayerUpdate = serde_json::from_value(json!({})).unwrap();
        assert_eq!(absent.end_time, None);
    }

    #[test]
    fn a_present_end_time_keeps_its_value() {
        let update: NexoraPlayerUpdate =
            serde_json::from_value(json!({ "endTime": 5000 })).unwrap();
        assert_eq!(update.end_time, Some(Some(5000)));
    }

    #[test]
    fn the_nested_track_block_keeps_its_field_names() {
        let update: NexoraPlayerUpdate =
            serde_json::from_value(json!({ "track": { "encoded": null, "userData": { "a": 1 } } }))
                .unwrap();
        let track = update.track.unwrap();
        assert_eq!(track.encoded, Some(None));
        assert_eq!(track.user_data.unwrap()["a"], 1);
    }
}
