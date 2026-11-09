use super::manager::error;
use scanner_types::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Phase {
    #[default]
    Open,
    Enabled,
    Ready,
    Transferring,
}
impl Phase {
    pub fn transfer_result(&mut self, status: u16) -> Result<()> {
        if !matches!(self, Self::Ready | Self::Transferring) {
            return Err(error("TWAIN transfer requested in the wrong state."));
        }
        match status {
            // TWRC_CANCEL also enters state 7 and requires ENDXFER, even when
            // cancellation happens on the first buffer (TWAIN spec §4).
            0 | 3 | 6 => *self = Self::Transferring,
            1 => {} // failure preserves state 6 for first buffer, otherwise 7
            _ => return Err(error("Unexpected TWAIN transfer status.")),
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_buffer_failure_and_cancellation_require_different_cleanup() {
        let mut phase = Phase::Ready;
        phase.transfer_result(1).unwrap();
        assert_eq!(phase, Phase::Ready);
        phase.transfer_result(3).unwrap();
        assert_eq!(phase, Phase::Transferring);
        phase.transfer_result(1).unwrap();
        assert_eq!(phase, Phase::Transferring);
        assert!(Phase::Open.transfer_result(0).is_err());
        assert!(Phase::Enabled.transfer_result(0).is_err());
    }
}
