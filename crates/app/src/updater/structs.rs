mod held;
mod no_driver;
#[cfg(target_os = "macos")]
mod native_relaunch;
#[cfg(target_os = "macos")]
mod sparkle_driver;
mod updater;
pub use held::Held;
pub use no_driver::NoDriver;
#[cfg(target_os = "macos")]
pub use native_relaunch::NativeRelaunch;
#[cfg(target_os = "macos")]
pub use sparkle_driver::SparkleDriver;
pub use updater::Updater;
