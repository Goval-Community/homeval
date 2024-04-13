use std::{fmt::Display, ops::Neg};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Session(pub i32);

static SESSION_ZERO: Session = Session(0);

impl Session {
    pub fn zero() -> Session {
        SESSION_ZERO
    }
}

impl Display for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Session {{ id: {} }}", self.0))
    }
}

impl Neg for Session {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl From<Session> for i32 {
    fn from(session: Session) -> Self {
        session.0
    }
}
