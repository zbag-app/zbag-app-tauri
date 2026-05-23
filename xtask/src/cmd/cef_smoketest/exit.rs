#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExitCode {
    Pass = 0,
    Policy = 1,
    Instrument = 2,
}

impl ExitCode {
    pub const fn code(self) -> u8 {
        self as u8
    }
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(value: ExitCode) -> Self {
        std::process::ExitCode::from(value.code())
    }
}

#[cfg(test)]
mod tests {
    use super::ExitCode;

    #[test]
    fn exit_codes_match_smoketest_contract() {
        assert_eq!(ExitCode::Pass.code(), 0);
        assert_eq!(ExitCode::Policy.code(), 1);
        assert_eq!(ExitCode::Instrument.code(), 2);
    }
}
