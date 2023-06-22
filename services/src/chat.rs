pub struct Chat {}
use super::traits;

impl traits::Service for Chat {
    fn attach(self, session: i32) -> Option<goval::Command> {
        None
    }
}
