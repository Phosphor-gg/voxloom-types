use serde::{Deserialize, Serialize};

/// A capability a customer has, resolved from Stripe entitlements rather than
/// from their tier.
///
/// The dashboard and the API both gate on these. Deriving a capability from
/// `Tier` instead would duplicate the mapping in two places, and the two
/// disagree the moment a feature moves product or is granted to a single
/// customer by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Entitlement {
    /// Speaker labels via diarisation.
    SpeakerDiarization,
    /// Filler-word and false-start removal.
    AiCleanup,
    /// Abstractive summary, key points, pull quotes and chapters.
    AiSummary,
    /// Translate a transcript into another language.
    Translation,
    /// Render a video with the subtitles burned into the frame.
    /// Media longer than the free tier's per-video ceiling, up to the 12 hour
    /// hard limit.
    LongForm,
    /// Submit a playlist or channel and get a job per video.
    BatchProcessing,
    /// Publicly hosted transcript pages.
    ShareLinks,
    /// REST API and the SDKs built on it.
    ApiAccess,
    /// Jobs jump the shared queue.
    PriorityQueue,
    /// Bias the decoder with a custom word list.
    CustomVocabulary,
}

impl Entitlement {
    /// The Stripe feature `lookup_key` this maps to.
    pub fn lookup_key(&self) -> &'static str {
        match self {
            Self::SpeakerDiarization => "speaker-diarization",
            Self::AiCleanup => "ai-cleanup",
            Self::AiSummary => "ai-summary",
            Self::Translation => "translation",
            Self::LongForm => "long-form",
            Self::BatchProcessing => "batch-processing",
            Self::ShareLinks => "share-links",
            Self::ApiAccess => "api-access",
            Self::PriorityQueue => "priority-queue",
            Self::CustomVocabulary => "custom-vocabulary",
        }
    }

    pub fn from_lookup_key(key: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|entitlement| entitlement.lookup_key() == key)
    }

    pub fn all() -> Vec<Entitlement> {
        vec![
            Self::SpeakerDiarization,
            Self::AiCleanup,
            Self::AiSummary,
            Self::Translation,
            Self::LongForm,
            Self::BatchProcessing,
            Self::ShareLinks,
            Self::ApiAccess,
            Self::PriorityQueue,
            Self::CustomVocabulary,
        ]
    }

    /// Label for a pricing card or a locked-feature tooltip.
    pub fn to_name(&self) -> &'static str {
        match self {
            Self::SpeakerDiarization => "Speaker labels",
            Self::AiCleanup => "AI cleanup",
            Self::AiSummary => "AI summary & chapters",
            Self::Translation => "Translation",
            Self::LongForm => "Long videos (up to 12 hours)",
            Self::BatchProcessing => "Playlists & channels",
            Self::ShareLinks => "Share links",
            Self::ApiAccess => "REST API & SDKs",
            Self::PriorityQueue => "Priority queue",
            Self::CustomVocabulary => "Custom vocabulary",
        }
    }

    /// Everything the free tier gets without a subscription.
    ///
    /// Deliberately not empty: a free account that cannot produce a usable
    /// transcript cannot decide whether to pay. It gets the balanced model and,
    /// like every plan, every export format; it is limited by minutes and by
    /// per-video length rather than by feature amputation.
    pub fn free_tier() -> Vec<Entitlement> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entitlement_round_trips_through_its_stripe_key() {
        for entitlement in Entitlement::all() {
            assert_eq!(
                Entitlement::from_lookup_key(entitlement.lookup_key()),
                Some(entitlement),
                "{entitlement:?}"
            );
        }
        assert_eq!(Entitlement::from_lookup_key("not-a-feature"), None);
    }

    #[test]
    fn lookup_keys_are_unique() {
        let mut keys: Vec<&str> = Entitlement::all().iter().map(|e| e.lookup_key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "two entitlements share a Stripe key");
    }
}
