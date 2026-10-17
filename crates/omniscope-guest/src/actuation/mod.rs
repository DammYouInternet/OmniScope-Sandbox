pub mod bezier_mouse;
pub mod driver;
pub mod error;
pub mod keyboard;

pub use bezier_mouse::{
    biometric_smootherstep, evaluate_cubic_bezier, generate_bezier_trajectory,
    generate_bezier_trajectory_with_rng, BezierTrajectory, Point, TrajectoryStep,
};
pub use driver::{InputDriver, MockInputDriver};
#[cfg(windows)]
pub use driver::WindowsInputDriver;
pub use error::ActuationError;
pub use keyboard::{plan_keystrokes, plan_keystrokes_with_rng, KeyAction, KeystrokePlan};
