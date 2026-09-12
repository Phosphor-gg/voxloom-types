use serde::{Deserialize, Serialize};

use crate::transcribe::{
    DEMO_ANON_WEEKLY_SECS, DEMO_AUTHED_WEEKLY_SECS, MediaMetadata, Transcript,
};

/// The free demo's allowance, per rolling seven-day window.
///
/// Anonymous callers are identified by a salted hash of their IP, signed-in
/// ones by user id. The signed-in allowance is six times larger because an
/// account is a much harder thing to farm than an address, and because the
/// upgrade from five minutes to thirty is the reason to sign up at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DemoIdentity {
    Anonymous,
    SignedIn,
}

impl DemoIdentity {
    pub fn weekly_allowance_secs(&self) -> i64 {
        match self {
            Self::Anonymous => DEMO_ANON_WEEKLY_SECS,
            Self::SignedIn => DEMO_AUTHED_WEEKLY_SECS,
        }
    }
}

/// What the demo allowance looks like right now.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DemoQuota {
    pub identity_kind: String,
    /// Seconds of media this identity may still transcribe in the window.
    pub remaining_secs: i64,
    pub allowance_secs: i64,
    pub used_secs: i64,
    /// RFC3339 instant the window rolls over.
    pub resets_at: String,
    /// Allowance a signed-in caller would have, so the anonymous demo can
    /// state the upgrade rather than hard-coding "30 minutes" in the UI.
    pub signed_in_allowance_secs: i64,
}

impl DemoQuota {
    pub fn is_exhausted(&self) -> bool {
        self.remaining_secs <= 0
    }

    /// Whether media of this length fits in what is left.
    pub fn allows(&self, duration_secs: f64) -> bool {
        (duration_secs.ceil() as i64) <= self.remaining_secs
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemoRequest {
    /// A YouTube URL, bare video id, or any direct media URL.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Ask for speaker labels. Granted on the demo so the feature can actually
    /// be evaluated; it is the single most common reason people pay.
    #[serde(default)]
    pub diarize: bool,
    /// Ask for a summary in the demo result.
    #[serde(default)]
    pub summarize: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemoResponse {
    pub transcript: Transcript,
    pub media: MediaMetadata,
    /// Quota state *after* this run, so the UI can update without a refetch.
    pub quota: DemoQuota,
    /// Seconds of media actually charged against the allowance.
    pub charged_secs: i64,
    /// True when the video was longer than the remaining allowance and only
    /// the first `charged_secs` were transcribed.
    pub truncated: bool,
}

/// Why a demo request was refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemoRefusal {
    pub reason: DemoRefusalReason,
    pub message: String,
    pub quota: DemoQuota,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemoRefusalReason {
    /// Window allowance spent.
    QuotaExhausted,
    /// Video longer than the whole allowance, so not even a truncated run
    /// would be useful.
    MediaTooLong,
    MediaUnavailable,
    UnsupportedSource,
}

impl DemoRefusalReason {
    /// Whether signing in would lift this refusal.
    pub fn signing_in_helps(&self) -> bool {
        matches!(self, Self::QuotaExhausted | Self::MediaTooLong)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_in_is_worth_six_times_the_anonymous_allowance() {
        assert_eq!(DemoIdentity::Anonymous.weekly_allowance_secs(), 5 * 60);
        assert_eq!(DemoIdentity::SignedIn.weekly_allowance_secs(), 30 * 60);
        assert_eq!(
            DemoIdentity::SignedIn.weekly_allowance_secs(),
            6 * DemoIdentity::Anonymous.weekly_allowance_secs()
        );
    }

    #[test]
    fn quota_admits_exactly_what_is_left_and_no_more() {
        let quota = DemoQuota {
            identity_kind: "anonymous".into(),
            remaining_secs: 120,
            allowance_secs: 300,
            used_secs: 180,
            resets_at: String::new(),
            signed_in_allowance_secs: 1800,
        };
        assert!(!quota.is_exhausted());
        assert!(quota.allows(120.0));
        assert!(quota.allows(119.4));
        assert!(!quota.allows(120.1));

        let spent = DemoQuota { remaining_secs: 0, used_secs: 300, ..quota };
        assert!(spent.is_exhausted());
        assert!(!spent.allows(1.0));
    }
}
