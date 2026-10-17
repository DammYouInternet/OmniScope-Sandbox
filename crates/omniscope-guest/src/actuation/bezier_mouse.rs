use rand::Rng;
use std::time::Duration;

/// 2D floating-point coordinate for Bézier calculations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// A single discrete step in a synthetic mouse trajectory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrajectoryStep {
    pub x: i32,
    pub y: i32,
    pub delay: Duration,
}

/// Complete planned trajectory for synthetic cursor motion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BezierTrajectory {
    pub steps: Vec<TrajectoryStep>,
    pub total_duration: Duration,
}

impl BezierTrajectory {
    /// Returns the final destination coordinate of the trajectory, or (0, 0) if empty.
    pub fn destination(&self) -> (i32, i32) {
        self.steps
            .last()
            .map(|s| (s.x, s.y))
            .unwrap_or((0, 0))
    }
}

/// Computes a point along a cubic Bézier curve defined by 4 control points at parameter t ∈ [0, 1].
pub fn evaluate_cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    let x = uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x;
    let y = uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y;

    Point::new(x, y)
}

/// 5th-order polynomial smootherstep function for natural biometric ease-in and ease-out:
/// S(t) = 6t^5 - 15t^4 + 10t^3.
pub fn biometric_smootherstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Generates a realistic, human-mimicking cursor trajectory between start and target.
///
/// Uses cubic Bézier interpolation with randomized control point deflections perpendicular
/// to the movement axis, combined with biometric ease-in / ease-out velocity distribution.
pub fn generate_bezier_trajectory(
    start: (i32, i32),
    target: (i32, i32),
    duration_ms: u32,
    step_interval_ms: u32,
) -> BezierTrajectory {
    let mut rng = rand::thread_rng();
    generate_bezier_trajectory_with_rng(start, target, duration_ms, step_interval_ms, &mut rng)
}

/// Generates a Bézier trajectory using a supplied random number generator for deterministic testing.
pub fn generate_bezier_trajectory_with_rng<R: Rng>(
    start: (i32, i32),
    target: (i32, i32),
    duration_ms: u32,
    step_interval_ms: u32,
    rng: &mut R,
) -> BezierTrajectory {
    let p0 = Point::new(start.0 as f64, start.1 as f64);
    let p3 = Point::new(target.0 as f64, target.1 as f64);
    let distance = p0.distance(&p3);

    // If start and target are identical or duration is 0, return immediate single step
    if distance < 1.0 || duration_ms == 0 {
        return BezierTrajectory {
            steps: vec![TrajectoryStep {
                x: target.0,
                y: target.1,
                delay: Duration::from_millis(duration_ms as u64),
            }],
            total_duration: Duration::from_millis(duration_ms as u64),
        };
    }

    let step_interval = step_interval_ms.max(5); // At least 5ms resolution
    let total_steps = (duration_ms / step_interval).max(2);
    let step_delay = Duration::from_millis(step_interval as u64);

    // Vector along line of travel
    let dx = p3.x - p0.x;
    let dy = p3.y - p0.y;

    // Perpendicular vector for natural human arc curvature
    let perp_x = -dy / distance;
    let perp_y = dx / distance;

    // Deflection magnitude (up to 15% of distance with randomized sign and amplitude)
    let deflection_p1 = rng.gen_range(-0.15..0.15) * distance;
    let deflection_p2 = rng.gen_range(-0.15..0.15) * distance;

    // Place control points roughly at 25% and 75% along travel line with perpendicular deflection
    let p1 = Point::new(
        p0.x + dx * 0.25 + perp_x * deflection_p1,
        p0.y + dy * 0.25 + perp_y * deflection_p1,
    );
    let p2 = Point::new(
        p0.x + dx * 0.75 + perp_x * deflection_p2,
        p0.y + dy * 0.75 + perp_y * deflection_p2,
    );

    let mut steps = Vec::with_capacity(total_steps as usize);

    for i in 1..=total_steps {
        let linear_t = i as f64 / total_steps as f64;
        let biometric_t = biometric_smootherstep(linear_t);
        let pt = evaluate_cubic_bezier(p0, p1, p2, p3, biometric_t);

        steps.push(TrajectoryStep {
            x: pt.x.round() as i32,
            y: pt.y.round() as i32,
            delay: step_delay,
        });
    }

    // Ensure the final step lands precisely on the exact target coordinates
    if let Some(last) = steps.last_mut() {
        last.x = target.0;
        last.y = target.1;
    }

    BezierTrajectory {
        steps,
        total_duration: Duration::from_millis(duration_ms as u64),
    }
}
