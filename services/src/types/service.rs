use crate::ChannelID;

#[derive(Clone, Debug)]
pub struct ServiceMetadata {
    pub service: String,
    pub name: Option<String>,
    pub id: ChannelID,
}
