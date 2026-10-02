/// The session, as the tracker links it.
pub struct SessionRef<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub agent: &'a str,
}
