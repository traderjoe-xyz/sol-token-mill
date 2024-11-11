use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct VestingPlan {
    pub stake_position: Pubkey,
    pub amount_vested: u64,
    pub amount_released: u64,
    pub start: i64,
    pub cliff_duration: i64,
    pub vesting_duration: i64,
}

impl VestingPlan {
    pub fn initialize(
        &mut self,
        stake_position: Pubkey,
        start: i64,
        amount_vested: u64,
        vesting_duration: i64,
        cliff_duration: i64,
    ) -> Result<()> {
        self.stake_position = stake_position;
        self.start = start;
        self.amount_vested = amount_vested;
        self.vesting_duration = vesting_duration;
        self.cliff_duration = cliff_duration;

        Ok(())
    }

    pub fn release(&mut self, current_time: i64) -> Result<u64> {
        let elapsed_time = current_time - self.start;

        if elapsed_time < self.cliff_duration {
            return Ok(0);
        }

        if elapsed_time >= self.vesting_duration {
            let amount_to_release = self.amount_vested - self.amount_released;
            self.amount_released += amount_to_release;

            return Ok(amount_to_release);
        }

        let amount_free =
            self.amount_vested * (elapsed_time as u64) / (self.vesting_duration as u64);
        let amount_to_release = amount_free - self.amount_released;
        self.amount_released += amount_to_release;

        Ok(amount_to_release)
    }
}
