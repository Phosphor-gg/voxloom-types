use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;

use crate::entitlements::Entitlement;
use crate::transcribe::{CREDITS_PER_MINUTE, TranscriptionModel, format_credits_as_time};

/// Length of the free trial, in days.
///
/// Shared so the number the pricing page promises is the number Stripe is told
/// to apply. Seven rather than three because the thing a trialist has to do is
/// find a video worth transcribing and check the transcript against it, and
/// that tends to happen at the weekend — a three day trial started on a
/// Tuesday expires before they ever try it properly. It costs nothing extra to
/// give: a trialist receives the plan's monthly allowance either way.
pub const TRIAL_PERIOD_DAYS: u32 = 7;

/// Credits per 1 GBP for pay-as-you-go minute packs.
///
/// 750 credits is 12.5 minutes, i.e. about £4.80 per hour of media.
/// Subscriptions are cheaper per minute than this on purpose: packs exist for
/// the month somebody overruns, not as the main way to buy.
///
/// This was 1,500 when a credit was half a second at the standard rate. It
/// halved with the model multiplier so that a pound still buys the same amount
/// of audio. Leaving it would have quietly cut pack prices in half, which a
/// test caught.
pub const CREDITS_PER_GBP: f64 = 750.0;

pub fn credits_to_gbp(credits: i64) -> f64 {
    credits as f64 / CREDITS_PER_GBP
}

pub fn format_credits_as_gbp(credits: i64) -> String {
    let gbp = credits_to_gbp(credits);
    let rounded_pence = (gbp * 100.0).round();
    if rounded_pence == rounded_pence.floor() && gbp >= 1.0 && (rounded_pence % 100.0) == 0.0 {
        format!("£{:.0}", gbp)
    } else {
        format!("£{:.2}", gbp)
    }
}

/// Minutes a credit balance buys at the standard rate.
///
/// Every customer-facing figure — plan cards, the balance in the dashboard
/// header, pack sizes — is quoted in *standard minutes*, meaning minutes on
/// [`TranscriptionModel::Weave`]. Quoting raw credits would be meaningless and
/// quoting minutes without naming the model would be a lie by omission, since
/// Tapestry consumes twice as many and Thread half as many.
pub fn credits_to_standard_minutes(credits: i64) -> i64 {
    credits.max(0) / CREDITS_PER_MINUTE
}

pub fn standard_minutes_to_credits(minutes: i64) -> i64 {
    minutes * CREDITS_PER_MINUTE
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditBalance {
    pub used_current_period: i64,
    pub max_monthly_credits: i64,
    pub remaining_credits: i64,
    pub usage_percentage: f64,
    pub reset_date: Option<NaiveDateTime>,
    pub extra_credits: i64,
}

impl CreditBalance {
    /// Remaining balance as human time, for the dashboard header.
    pub fn remaining_display(&self) -> String {
        format_credits_as_time(self.remaining_credits)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditTransaction {
    pub id: String,
    pub amount: i64,
    pub transaction_type: String,
    pub model_type: Option<String>,
    /// Seconds of media this transaction covered, for usage rows that came
    /// from a transcription job.
    pub media_seconds: Option<i64>,
    pub description: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StripeSubscriptionInfo {
    pub tier: Tier,
    pub billing_cycle: BillingCycle,
    pub status: String,
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub is_active: bool,
    /// Price in the smallest currency unit (e.g. pence).
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub payment_method_id: Option<String>,
    #[serde(default)]
    pub subscription_id: Option<String>,
    #[serde(default)]
    pub overdraft_enabled: bool,
    #[serde(default)]
    pub overdraft_limit: Option<i64>,
    #[serde(default)]
    pub overdraft_used: Option<i64>,
    /// Stripe entitlements for this customer. The dashboard gates features on
    /// these rather than on `tier`, matching what the API actually enforces.
    #[serde(default)]
    pub entitlements: Vec<Entitlement>,
}

impl StripeSubscriptionInfo {
    pub fn free() -> Self {
        Self {
            tier: Tier::Free,
            billing_cycle: BillingCycle::Monthly,
            status: "inactive".to_string(),
            current_period_end: None,
            cancel_at_period_end: false,
            is_active: false,
            price: None,
            currency: None,
            payment_method_id: None,
            subscription_id: None,
            overdraft_enabled: false,
            overdraft_limit: None,
            overdraft_used: None,
            entitlements: Entitlement::free_tier(),
        }
    }

    pub fn has(&self, entitlement: Entitlement) -> bool {
        self.entitlements.contains(&entitlement)
    }

    /// Models this customer may ask for, best first. Drives the model picker
    /// and `Auto` resolution.
    pub fn available_models(&self) -> Vec<TranscriptionModel> {
        TranscriptionModel::concrete_models()
            .into_iter()
            .filter(|model| model.entitlement().is_none_or(|e| self.has(e)))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverdraftSettingsRequest {
    pub enabled: bool,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverdraftSettingsResponse {
    pub enabled: bool,
    pub limit: i64,
    pub used: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub tier: Tier,
    pub cycle: BillingCycle,
    pub price: f64,
    pub expires_at: Option<NaiveDateTime>,
    pub max_monthly_credits: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditsInfoResponse {
    pub balance: i64,
    pub monthly_allocation: i64,
    pub used_this_month: i64,
    pub remaining_this_month: i64,
    pub usage_percentage: f64,
    #[serde(default)]
    pub reset_date: Option<NaiveDateTime>,
    #[serde(default)]
    pub extra_credits: i64,
    #[serde(default)]
    pub discount_available: bool,
    #[serde(default)]
    pub discount_expires_at: Option<String>,
    /// Percentage value of the personal discount (e.g. 10 means 10% off).
    /// Only meaningful when `discount_available` is true.
    #[serde(default)]
    pub discount_percentage: Option<u32>,
    /// Whether the user has exhausted their current short-window rate limit.
    #[serde(default)]
    pub rate_limited: bool,
    #[serde(default)]
    pub rate_limit_resets_at: Option<String>,
    /// Whether the free trial can still be started. Length is
    /// [`TRIAL_PERIOD_DAYS`].
    #[serde(default)]
    pub trial_available: bool,
}

impl CreditsInfoResponse {
    pub fn remaining_display(&self) -> String {
        format_credits_as_time(self.balance)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyUsageEntry {
    pub day: String,
    pub credits_used: i64,
    /// Jobs finished that day. Shown alongside minutes so a spike is
    /// attributable to one long video rather than to many short ones.
    #[serde(default)]
    pub jobs: i64,
}

#[derive(Debug, Deserialize)]
pub struct DailyUsageQuery {
    pub days: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCheckoutSessionRequest {
    pub tier: Tier,
    pub billing_cycle: BillingCycle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral_code: Option<String>,
    /// Start a free trial instead of an immediate charge: [`TRIAL_PERIOD_DAYS`]
    /// days, card required, converting to a normal recurring subscription when
    /// it ends. Only for accounts with no active subscription, and only on
    /// cycles where [`BillingCycle::trial_eligible`] holds.
    #[serde(default)]
    pub trial: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCheckoutSessionResponse {
    pub checkout_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionUpdateResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePlanRequest {
    pub tier: Tier,
    pub billing_cycle: BillingCycle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethodResponse {
    pub last4: String,
    pub brand: String,
    pub exp_month: u32,
    pub exp_year: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BillingPortalResponse {
    pub portal_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToggleAutoRenewalRequest {
    pub enable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToggleAutoRenewalResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceInfo {
    pub price_id: String,
    pub product_id: String,
    pub tier: Tier,
    pub billing_cycle: BillingCycle,
    /// Amount in the smallest currency unit.
    pub amount: i64,
    pub currency: String,
    pub payment_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormattedPrice {
    pub price_id: String,
    pub amount_cents: i64,
    /// Amount in major currency units.
    pub amount_display: f64,
    pub currency: String,
}

impl FormattedPrice {
    pub fn currency_symbol(&self) -> &str {
        match self.currency.to_uppercase().as_str() {
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "JPY" => "¥",
            "CAD" => "CA$",
            "AUD" => "A$",
            "CHF" => "CHF ",
            "CNY" => "¥",
            "INR" => "₹",
            _ => &self.currency,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierPricing {
    pub tier: Tier,
    pub prices: HashMap<BillingCycle, FormattedPrice>,
    #[serde(default)]
    pub monthly_credits: Option<i64>,
    #[serde(default)]
    pub feature_names: Vec<String>,
}

impl TierPricing {
    pub fn description(&self) -> String {
        self.tier.get_description().to_string()
    }

    pub fn features(&self) -> Vec<String> {
        if !self.feature_names.is_empty() {
            self.feature_names.clone()
        } else {
            self.tier.get_features()
        }
    }

    /// Standard minutes this plan includes per period, for the headline figure
    /// on a pricing card.
    pub fn included_minutes(&self) -> i64 {
        credits_to_standard_minutes(self.monthly_credits.unwrap_or_else(|| self.tier.monthly_credits()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlashSaleInfo {
    pub id: String,
    pub name: String,
    pub discount_percentage: i32,
    pub eligible_tiers: Option<Vec<Tier>>,
    pub ends_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricingData {
    pub tiers: Vec<TierPricing>,
    pub billing_cycles: Vec<BillingCycleInfo>,
    #[serde(default)]
    pub flash_sale: Option<FlashSaleInfo>,
    #[serde(default)]
    pub free_tier_credits: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillingCycleInfo {
    pub cycle: BillingCycle,
    pub display_name: String,
    pub period_suffix: String,
    pub discount_percentage: Option<i32>,
}

/// Subscription tiers.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Solo,
    Studio,
    Agency,
}

impl Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tier_str = match self {
            Tier::Free => "Free",
            Tier::Solo => "Solo",
            Tier::Studio => "Studio",
            Tier::Agency => "Agency",
        };
        write!(f, "{}", tier_str)
    }
}

impl From<String> for Tier {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl From<&str> for Tier {
    /// Parse a tier from any spelling that shows up in Stripe metadata, a
    /// query string or a stored subscription.
    ///
    /// `+` is normalised to `plus` rather than stripped: stripping it turns
    /// "creator+" into "creator" and silently downgrades the customer to the
    /// tier below the one they bought.
    fn from(s: &str) -> Self {
        let normalised = s.to_lowercase().replace('+', "plus").replace(['_', '-', ' '], "");
        match normalised.as_str() {
            "free" => Tier::Free,
            "solo" => Tier::Solo,
            "studio" => Tier::Studio,
            "agency" => Tier::Agency,
            _ => Tier::Free,
        }
    }
}

impl Tier {
    /// Every tier, cheapest first.
    pub fn all_tiers() -> Vec<Tier> {
        vec![Tier::Free, Tier::Solo, Tier::Studio, Tier::Agency]
    }

    /// Tiers rendered on the pricing page as buyable.
    pub fn sellable_tiers() -> Vec<Tier> {
        vec![Tier::Solo, Tier::Studio, Tier::Agency]
    }

    /// The tier a pricing page highlights. Studio rather than the cheapest
    /// paid plan: it is the first tier with speaker labels and long videos,
    /// which together are what most people are actually shopping for.
    pub fn most_popular() -> Tier {
        Tier::Studio
    }

    /// Stripe product `lookup_key` stem for this tier.
    pub fn lookup_key(&self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Solo => "solo",
            Tier::Studio => "studio",
            Tier::Agency => "agency",
        }
    }

    pub fn get_description(&self) -> &str {
        match self {
            Tier::Free => "Ten minutes a month to see whether the transcript holds up.",
            Tier::Solo => "For one person keeping up with their own channel.",
            Tier::Studio => "For interviews, podcasts and long-form, at volume.",
            Tier::Agency => "For teams and agencies running everything through the API.",
        }
    }

    /// Credits included per month.
    ///
    /// Quoted to customers as standard minutes via
    /// [`credits_to_standard_minutes`]: 10 minutes, 5 hours, 20 hours, 100 hours.
    pub fn monthly_credits(&self) -> i64 {
        match self {
            Tier::Free => standard_minutes_to_credits(10),
            Tier::Solo => standard_minutes_to_credits(5 * 60),
            Tier::Studio => standard_minutes_to_credits(20 * 60),
            Tier::Agency => standard_minutes_to_credits(100 * 60),
        }
    }

    /// Longest single video this tier accepts, in seconds.
    ///
    /// The free tier's ten-minute ceiling is the same shape the incumbent
    /// uses, and it exists because a free account that can transcribe one
    /// three-hour video has been given the product rather than a trial of it.
    pub fn max_video_secs(&self) -> f64 {
        match self {
            Tier::Free => 10.0 * 60.0,
            _ => crate::transcribe::MAX_MEDIA_DURATION_SECS,
        }
    }

    /// Entitlements this tier grants. Mirrored into Stripe as the product's
    /// features; this is the source of truth the seed script writes from.
    pub fn entitlements(&self) -> Vec<Entitlement> {
        use Entitlement as E;
        match self {
            Tier::Free => E::free_tier(),
            Tier::Solo => vec![
                // Speaker labels belong on the entry paid plan, not above it:
                // interviews and podcasts are the common case in this niche,
                // and a plan that cannot tell two voices apart does not solve
                // the problem those customers are paying to solve.
                E::SpeakerDiarization,
                E::AiCleanup,
                E::AiSummary,
                E::Translation,
                E::LongForm,
                E::ShareLinks,
            ],
            Tier::Studio => vec![
                E::SpeakerDiarization,
                E::AiCleanup,
                E::AiSummary,
                E::Translation,
                E::LongForm,
                E::BatchProcessing,
                E::ShareLinks,
                E::ApiAccess,
                E::CustomVocabulary,
            ],
            Tier::Agency => Entitlement::all(),
        }
    }

    pub fn has(&self, entitlement: Entitlement) -> bool {
        self.entitlements().contains(&entitlement)
    }

    /// The feature list shown on a pricing card.
    ///
    /// Built from [`Tier::entitlements`] rather than written out, so a card can
    /// never promise a feature the tier does not actually grant. The first two
    /// lines are the allowance and the "everything in the tier below" roll-up,
    /// which are what people compare.
    pub fn get_features(&self) -> Vec<String> {
        let mut features = Vec::new();
        let minutes = credits_to_standard_minutes(self.monthly_credits());
        features.push(match self {
            // Not "one time": the free allowance resets with everyone else's,
            // on the 30 day cycle in credits_service::check_and_reset_if_needed.
            // Saying it never comes back understated the free tier and
            // contradicted the same page's own subheading.
            Tier::Free => format!("{} minutes of transcription a month", minutes),
            _ if minutes >= 60 => format!("{} hours of transcription a month", minutes / 60),
            _ => format!("{} minutes of transcription a month", minutes),
        });

        let inherited: Vec<Entitlement> = match self {
            Tier::Free => Vec::new(),
            Tier::Solo => Tier::Free.entitlements(),
            Tier::Studio => Tier::Solo.entitlements(),
            Tier::Agency => Tier::Studio.entitlements(),
        };

        if let Some(below) = self.tier_below() {
            features.push(format!("Everything in {}", below));
        }

        for entitlement in self.entitlements() {
            if inherited.contains(&entitlement) {
                continue;
            }
            features.push(entitlement.to_name().to_string());
        }

        if matches!(self, Tier::Free) {
            features.push("Videos up to 10 minutes".to_string());
            features.push("TXT and SRT export".to_string());
        } else {
            features.push("Every export format".to_string());
        }

        features
    }

    fn tier_below(&self) -> Option<Tier> {
        match self {
            Tier::Free => None,
            Tier::Solo => Some(Tier::Free),
            Tier::Studio => Some(Tier::Solo),
            Tier::Agency => Some(Tier::Studio),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Copy, Serialize, Deserialize, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BillingCycle {
    Monthly,
    Quarterly,
    Annual,
    Triennial,
}

impl BillingCycle {
    /// Whether a free trial can be started on this cycle.
    ///
    /// Triennial is excluded deliberately: a free week converting into a three
    /// year commitment is the shape most likely to end in a chargeback, and
    /// the trial exists to de-risk trying Voxloom, not to front a long
    /// contract.
    ///
    /// Lives here rather than in the checkout route because the pricing page
    /// decides which cards offer a trial and the backend decides whether to
    /// grant one. If those disagree, a customer is either shown an offer that
    /// is refused or charged when they expected a trial.
    pub fn trial_eligible(&self) -> bool {
        !matches!(self, BillingCycle::Triennial)
    }

    /// Months covered by one billing period.
    ///
    /// This is the ordering used to tell an upgrade from a downgrade: a longer
    /// term is an upgrade and prorates immediately, a shorter one is a
    /// downgrade and takes effect when the paid term ends. The money runs the
    /// other way (a longer term is cheaper per month), so comparing prices
    /// instead would call shortening a commitment an upgrade and bill for it.
    pub fn months(&self) -> u32 {
        match self {
            BillingCycle::Monthly => 1,
            BillingCycle::Quarterly => 3,
            BillingCycle::Annual => 12,
            BillingCycle::Triennial => 36,
        }
    }

    pub fn is_downgrade_to(&self, target: BillingCycle) -> bool {
        target.months() < self.months()
    }

    pub fn period_suffix(&self) -> &str {
        match self {
            BillingCycle::Monthly => "month",
            BillingCycle::Quarterly => "3 months",
            BillingCycle::Annual => "year",
            BillingCycle::Triennial => "3 years",
        }
    }

    pub fn get_all_cycles() -> Vec<BillingCycle> {
        vec![
            BillingCycle::Monthly,
            BillingCycle::Quarterly,
            BillingCycle::Annual,
            BillingCycle::Triennial,
        ]
    }

    pub fn stripe_interval(&self) -> &'static str {
        match self {
            BillingCycle::Monthly | BillingCycle::Quarterly => "month",
            BillingCycle::Annual | BillingCycle::Triennial => "year",
        }
    }

    pub fn stripe_interval_count(&self) -> u32 {
        match self {
            BillingCycle::Monthly => 1,
            BillingCycle::Quarterly => 3,
            BillingCycle::Annual => 1,
            BillingCycle::Triennial => 3,
        }
    }
}

impl Display for BillingCycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cycle_str = match self {
            BillingCycle::Monthly => "Monthly",
            BillingCycle::Quarterly => "Quarterly",
            BillingCycle::Annual => "Annual",
            BillingCycle::Triennial => "Triennial",
        };
        write!(f, "{}", cycle_str)
    }
}

impl From<String> for BillingCycle {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl From<&str> for BillingCycle {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "monthly" => BillingCycle::Monthly,
            "quarterly" => BillingCycle::Quarterly,
            "annual" | "yearly" => BillingCycle::Annual,
            "triennial" => BillingCycle::Triennial,
            _ => BillingCycle::Monthly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferralStats {
    pub referral_code: String,
    pub total_referrals: usize,
    /// Referrals that have paid out, i.e. the referred user subscribed.
    pub rewarded_referrals: usize,
    /// Credits earned from referrals, all time. Render with
    /// [`format_credits_as_time`]; the raw credit figure is never shown.
    pub total_reward_credits: i64,
    pub referral_breakdown: Vec<ReferralInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferralInfo {
    pub user_id: String,
    pub created_at: String,
    /// Credits this referral paid out, or 0 while it is still pending.
    pub reward_credits: i64,
    /// Whether the reward has been paid. A referral pays out once, when the
    /// referred user first subscribes, so this never returns to false even if
    /// they later cancel.
    pub rewarded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoiceInfo {
    pub id: String,
    pub amount: i64,
    pub currency: String,
    pub status: String,
    pub created: String,
    pub hosted_invoice_url: Option<String>,
    pub invoice_pdf: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvoicesResponse {
    pub invoices: Vec<InvoiceInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cycle_but_triennial_can_be_trialled() {
        assert!(BillingCycle::Monthly.trial_eligible());
        assert!(BillingCycle::Quarterly.trial_eligible());
        assert!(BillingCycle::Annual.trial_eligible());
        assert!(!BillingCycle::Triennial.trial_eligible());
    }

    #[test]
    fn direction_follows_term_length_not_price() {
        // Triennial is the cheapest per month, so a price comparison would call
        // this an upgrade. It shortens the commitment, so it is a downgrade.
        assert!(BillingCycle::Triennial.is_downgrade_to(BillingCycle::Annual));
        assert!(BillingCycle::Annual.is_downgrade_to(BillingCycle::Monthly));
        assert!(!BillingCycle::Monthly.is_downgrade_to(BillingCycle::Annual));
        assert!(!BillingCycle::Annual.is_downgrade_to(BillingCycle::Annual));
    }

    #[test]
    fn tiers_parse_from_every_spelling_stripe_metadata_might_hold() {
        assert_eq!(Tier::from("solo"), Tier::Solo);
        assert_eq!(Tier::from("Studio"), Tier::Studio);
        assert_eq!(Tier::from("STUDIO"), Tier::Studio);
        assert_eq!(Tier::from("agency"), Tier::Agency);
        assert_eq!(Tier::from(" agency "), Tier::Agency);
        // An unknown tier must fall back to the least privileged one, never
        // grant something by accident.
        assert_eq!(Tier::from("enterprise-2024"), Tier::Free);
    }

    #[test]
    fn every_tier_round_trips_through_serde() {
        // Tiers are HashMap keys in PricingData and values in Stripe
        // metadata. A serde name that does not match the lookup key means the
        // pricing snapshot fails to deserialise and the page shows skeletons.
        for tier in Tier::all_tiers() {
            let wire = serde_json::to_string(&tier).unwrap();
            let back: Tier = serde_json::from_str(&wire).unwrap();
            assert_eq!(back, tier);
            assert_eq!(
                wire.trim_matches('"'),
                tier.lookup_key(),
                "{tier:?} serialises as {wire} but its lookup key is {:?}",
                tier.lookup_key()
            );
        }
    }

    #[test]
    fn every_tier_round_trips_through_its_stripe_lookup_key() {
        // The lookup key is what Stripe metadata carries, so a tier that does
        // not parse back from its own key silently downgrades a paying
        // customer to Free.
        for tier in Tier::all_tiers() {
            assert_eq!(
                Tier::from(tier.lookup_key()),
                tier,
                "{tier:?} does not survive its own lookup key"
            );
        }
    }

    #[test]
    fn tier_names_and_keys_are_distinct() {
        let mut keys: Vec<&str> = Tier::all_tiers().iter().map(|t| t.lookup_key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "two tiers share a Stripe lookup key");
    }

    #[test]
    fn allowances_and_entitlements_both_grow_with_the_tier() {
        let tiers = Tier::all_tiers();
        for pair in tiers.windows(2) {
            assert!(
                pair[0].monthly_credits() < pair[1].monthly_credits(),
                "{} !< {}",
                pair[0],
                pair[1]
            );
            assert!(
                pair[0].entitlements().len() <= pair[1].entitlements().len(),
                "{} grants more than {}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn a_tier_never_loses_an_entitlement_the_tier_below_it_had() {
        for pair in Tier::all_tiers().windows(2) {
            for entitlement in pair[0].entitlements() {
                assert!(
                    pair[1].has(entitlement),
                    "{} has {:?} but {} does not",
                    pair[0],
                    entitlement,
                    pair[1]
                );
            }
        }
    }

    #[test]
    fn plan_allowances_are_the_round_numbers_the_cards_promise() {
        assert_eq!(credits_to_standard_minutes(Tier::Free.monthly_credits()), 10);
        assert_eq!(credits_to_standard_minutes(Tier::Solo.monthly_credits()), 300);
        assert_eq!(credits_to_standard_minutes(Tier::Studio.monthly_credits()), 1200);
        assert_eq!(credits_to_standard_minutes(Tier::Agency.monthly_credits()), 6000);
    }

    #[test]
    fn a_card_never_lists_a_feature_its_tier_does_not_grant() {
        for tier in Tier::all_tiers() {
            let features = tier.get_features();
            let granted: Vec<&str> = tier.entitlements().iter().map(|e| e.to_name()).collect();
            for line in &features {
                // Lines are either the allowance, the roll-up, an export note,
                // or the name of an entitlement this tier actually has.
                let is_prose = line.contains("transcription a month")
                    || line.contains("one time")
                    || line.starts_with("Everything in")
                    || line.contains("export")
                    || line.contains("Videos up to");
                assert!(
                    is_prose || granted.contains(&line.as_str()),
                    "{tier} card promises {line:?} which it does not grant"
                );
            }
        }
    }

    #[test]
    fn free_accounts_cannot_transcribe_a_feature_film() {
        assert_eq!(Tier::Free.max_video_secs(), 600.0);
        for tier in Tier::sellable_tiers() {
            assert_eq!(tier.max_video_secs(), crate::transcribe::MAX_MEDIA_DURATION_SECS);
        }
    }

    #[test]
    fn the_free_tier_can_still_produce_a_transcript_worth_judging() {
        // A free tier with no model is a signup form, not a trial.
        let free = StripeSubscriptionInfo::free();
        assert!(!free.available_models().is_empty());
        // But it must not hand out the things people pay for.
        assert!(!free.has(Entitlement::SpeakerDiarization));
        assert!(!free.has(Entitlement::ApiAccess));
    }

    #[test]
    fn packs_are_priced_above_the_cheapest_subscription_minute() {
        // £1 of pay-as-you-go.
        let pack_minutes = credits_to_standard_minutes(CREDITS_PER_GBP as i64);
        assert_eq!(pack_minutes, 12);

        // Solo is £9 for 300 minutes if the Stripe price matches the card,
        // i.e. ~33 minutes per pound — comfortably better than a pack, which
        // is the incentive to subscribe rather than top up forever.
        assert!(pack_minutes < 33);
    }
}
