use std::fmt::Display;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct ChannelID(pub i32);

impl Display for ChannelID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Channel {{ id: {} }}", self.0))
    }
}

impl From<ChannelID> for i32 {
    fn from(channel: ChannelID) -> Self {
        channel.0
    }
}
