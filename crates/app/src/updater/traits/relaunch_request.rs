/// The updater, waiting for the app to say whether it may restart so that an update installs.
pub trait RelaunchRequest {
    /// The app may restart: the update installs.
    fn proceed(self: Box<Self>);
    /// The app stays as it is: the reader said no.
    fn decline(self: Box<Self>);
}
