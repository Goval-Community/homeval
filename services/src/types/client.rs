#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub is_secure: bool,

    pub username: String,
    pub id: u32,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            is_secure: false,

            username: "homeval-user".to_owned(),
            id: 23054564,
        }
    }
}
