#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_claim() {
        let mut contract = ReferralContract::new(50, 25);
        contract.register_referral("alice".into(), "bob".into()).unwrap();
        assert!(contract.claim_referral_reward("bob".into(), 123456).is_ok());
    }

    #[test]
    fn test_duplicate_claim_rejected() {
        let mut contract = ReferralContract::new(50, 25);
        contract.register_referral("alice".into(), "bob".into()).unwrap();
        contract.claim_referral_reward("bob".into(), 123456).unwrap();
        assert!(contract.claim_referral_reward("bob".into(), 123457).is_err());
    }

    #[test]
    fn test_stats_accuracy() {
        let mut contract = ReferralContract::new(50, 25);
        contract.register_referral("alice".into(), "bob".into()).unwrap();
        contract.claim_referral_reward("bob".into(), 123456).unwrap();
        let stats = contract.get_referral_stats("alice".into());
        assert_eq!(stats.0, 1);
        assert_eq!(stats.1, 50);
    }
}
