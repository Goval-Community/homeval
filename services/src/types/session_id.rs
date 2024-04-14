use std::{fmt::Display, ops::Neg};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SessionID(pub i32);

static SESSION_ZERO: SessionID = SessionID(0);

impl SessionID {
    pub fn zero() -> SessionID {
        SESSION_ZERO
    }
}

impl Display for SessionID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("SessionID {{ id: {} }}", self.0))
    }
}

impl Neg for SessionID {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl From<SessionID> for i32 {
    fn from(session: SessionID) -> Self {
        session.0
    }
}
