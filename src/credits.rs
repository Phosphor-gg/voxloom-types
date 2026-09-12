use serde::{Deserialize, Serialize};

/// A one-off pack of minutes, sold as a Stripe one-time price.
///
/// Packs top up `extra_credits`, which is drawn down *before* the monthly
/// allowance. Spending the perishable balance first is the only ordering that
/// does not quietly expire minutes a customer paid cash for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditProductResponse {
    pub id: String,
    pub price_id: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i64,
    pub currency: String,
    /// Credits granted. Rendered as standard minutes via
    /// [`crate::pricing::credits_to_standard_minutes`].
    pub credits_amount: i64,
}

impl CreditProductResponse {
    /// Standard minutes this pack is worth.
    pub fn minutes(&self) -> i64 {
        crate::pricing::credits_to_standard_minutes(self.credits_amount)
    }

    /// Price per standard hour, for comparing packs against each other.
    pub fn price_per_hour_cents(&self) -> Option<i64> {
        let minutes = self.minutes();
        if minutes <= 0 {
            return None;
        }
        Some(self.price_cents * 60 / minutes)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyCreditsRequest {
    pub price_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyCreditsResponse {
    pub checkout_url: String,
}
