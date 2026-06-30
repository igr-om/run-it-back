/// Chip accounting for the abstracted subgame, all in big blinds. `invested`
/// tracks *this player's total contribution this hand* (not just this
/// street) so showdown payoffs net out correctly against the real stack.
#[derive(Debug, Clone, Copy)]
pub struct PotState {
    pub hero_invested: f32,
    pub villain_invested: f32,
    pub effective_stack: f32,
}

impl PotState {
    pub fn pot(&self) -> f32 {
        self.hero_invested + self.villain_invested
    }

    pub fn hero_stack_left(&self) -> f32 {
        self.effective_stack - self.hero_invested
    }

    pub fn villain_stack_left(&self) -> f32 {
        self.effective_stack - self.villain_invested
    }

    /// Apply a hero bet/raise/call/allin that brings hero's *total invested
    /// this street+hand* up to `to_amount` (clamped to the stack).
    pub fn with_hero_to(&self, to_amount: f32) -> PotState {
        let mut s = *self;
        s.hero_invested = to_amount.min(self.effective_stack);
        s
    }

    pub fn with_villain_to(&self, to_amount: f32) -> PotState {
        let mut s = *self;
        s.villain_invested = to_amount.min(self.effective_stack);
        s
    }
}
