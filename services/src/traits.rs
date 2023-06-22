pub(crate) trait Service {
    fn attach(self, session: i32) -> Option<goval::Command>;
}
