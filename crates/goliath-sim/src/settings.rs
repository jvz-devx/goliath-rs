//! Owns explicit failure-detection delay and deliberate protocol mutations.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mutation {
    #[default]
    None,
    P2EarlyCommit,
    P2SplitSave,
    OlderStoreRecovery,
    P2MissingInstall,
    UnfencedCas,
    P5EarlyCommit,
    P5SkipChest,
    P5ReplayPastTorn,
    P5StaleAppend,
}

impl Mutation {
    pub const CASES: [(Self, goliath_model::Policy); 9] = [
        (Self::P2EarlyCommit, goliath_model::Policy::P2),
        (Self::P2SplitSave, goliath_model::Policy::P2),
        (Self::OlderStoreRecovery, goliath_model::Policy::P2),
        (Self::P2MissingInstall, goliath_model::Policy::P2),
        (Self::UnfencedCas, goliath_model::Policy::P2),
        (Self::P5EarlyCommit, goliath_model::Policy::P5),
        (Self::P5SkipChest, goliath_model::Policy::P5),
        (Self::P5ReplayPastTorn, goliath_model::Policy::P5),
        (Self::P5StaleAppend, goliath_model::Policy::P5),
    ];
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub abort_delay_ms: u64,
    pub mutation: Mutation,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            abort_delay_ms: 40,
            mutation: Mutation::None,
        }
    }
}
