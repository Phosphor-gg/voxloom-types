use serde::{Deserialize, Serialize};

use crate::transcribe::{MediaMetadata, Transcript};

/// A publicly hosted transcript page.
///
/// Sharing is opt-in per job and revocable. Nothing about an account is
/// exposed: a share carries the transcript and the media it came from, never
/// the owner, because the common use is pasting the link into a public thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Share {
    /// URL-safe slug the page is served at.
    pub slug: String,
    pub job_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Whether search engines may index the page. Off by default: a shared
    /// transcript is usually shared with a person, not with the web.
    #[serde(default)]
    pub indexable: bool,
    /// Hide the timestamps on the public page.
    #[serde(default)]
    pub hide_timestamps: bool,
    /// Show the AI summary above the transcript, when one exists.
    #[serde(default)]
    pub show_summary: bool,
    pub view_count: i64,
    /// RFC3339. `None` means the link does not expire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    pub created_at: String,
}

impl Share {
    pub fn url(&self, origin: &str) -> String {
        format!("{}/t/{}", origin.trim_end_matches('/'), self.slug)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShareRequest {
    pub job_id: String,
    #[serde(default)]
    pub indexable: bool,
    #[serde(default)]
    pub hide_timestamps: bool,
    #[serde(default = "default_true")]
    pub show_summary: bool,
    /// Days until the link stops working. `None` for no expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_days: Option<u32>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateShareRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_summary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareResponse {
    pub share: Share,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharesResponse {
    pub shares: Vec<Share>,
}

/// Everything a public transcript page renders, in one unauthenticated read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicTranscriptResponse {
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub media: MediaMetadata,
    pub transcript: Transcript,
    pub hide_timestamps: bool,
    pub show_summary: bool,
    pub indexable: bool,
    pub created_at: String,
}
