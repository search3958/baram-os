//! Stateful pointer acceleration and click-jitter suppression.
//!
//! Quadratic transfer curves provide low-speed precision and fast travel
//! without a discontinuity between the two regions.

#[derive(Clone, Copy, Debug)]
pub struct MotionSettings {
    /// Master multiplier. 1.0 preserves the default medium-speed feel.
    pub speed: f32,
    /// Gain approached by very slow motion.
    pub base_gain: f32,
    /// Adds gain in proportion to speed, making output approximately x^2.
    pub quadratic_gain: f32,
    pub max_gain: f32,
    /// Maximum coincident motion suppressed around a button transition.
    pub click_guard_distance: f32,
    pub click_guard_reports: u8,
    /// Fraction of the previous speed retained; lower reacts more quickly.
    pub smoothing: f32,
}

impl Default for MotionSettings {
    fn default() -> Self {
        Self {
            speed: 1.0,
            base_gain: 0.12,
            quadratic_gain: 0.004,
            max_gain: 4.0,
            click_guard_distance: 3.0,
            click_guard_reports: 3,
            smoothing: 0.10,
        }
    }
}

impl MotionSettings {
    pub fn sanitized(mut self) -> Self {
        self.speed = self.speed.clamp(0.10, 4.0);
        self.base_gain = self.base_gain.clamp(0.0, 1.0);
        self.quadratic_gain = self.quadratic_gain.clamp(0.0, 0.05);
        self.max_gain = self.max_gain.clamp(0.25, 8.0);
        self.click_guard_distance = self.click_guard_distance.clamp(0.0, 8.0);
        self.click_guard_reports = self.click_guard_reports.min(8);
        self.smoothing = self.smoothing.clamp(0.0, 0.95);
        self
    }

    pub fn gain(&self, speed: f32) -> f32 {
        ((self.base_gain + self.quadratic_gain * speed.max(0.0)) * self.speed).min(self.max_gain)
    }
}

#[derive(Debug, Default)]
pub struct MotionFilter {
    fraction_x: f32,
    fraction_y: f32,
    filtered_speed: f32,
    buttons: u8,
    click_guard_remaining: u8,
}

impl MotionFilter {
    pub fn apply(
        &mut self,
        dx: i32,
        dy: i32,
        buttons: u8,
        settings: &MotionSettings,
    ) -> (i32, i32) {
        let settings = settings.sanitized();
        let button_changed = buttons != self.buttons;
        self.buttons = buttons;
        if button_changed {
            self.click_guard_remaining = settings.click_guard_reports;
            // Do not carry a half-pixel from positioning into the click.
            self.fraction_x = 0.0;
            self.fraction_y = 0.0;
        }

        let raw_x = dx as f32;
        let raw_y = dy as f32;
        let abs_x = raw_x.abs();
        let abs_y = raw_y.abs();
        // Accurate enough for curve selection, with no sqrt dependency.
        let (major, minor) = if abs_x >= abs_y {
            (abs_x, abs_y)
        } else {
            (abs_y, abs_x)
        };
        let instant_speed = major + minor * 0.375;

        if self.click_guard_remaining != 0 {
            self.click_guard_remaining -= 1;
            if instant_speed <= settings.click_guard_distance {
                self.filtered_speed *= 0.5;
                return (0, 0);
            }
        }
        if dx == 0 && dy == 0 {
            self.filtered_speed *= settings.smoothing;
            return (0, 0);
        }

        self.filtered_speed =
            self.filtered_speed * settings.smoothing + instant_speed * (1.0 - settings.smoothing);
        let gain = settings.gain(self.filtered_speed);
        let scaled_x = raw_x * gain + self.fraction_x;
        let scaled_y = raw_y * gain + self.fraction_y;
        let output_x = scaled_x as i32;
        let output_y = scaled_y as i32;
        self.fraction_x = scaled_x - output_x as f32;
        self.fraction_y = scaled_y - output_y as f32;
        (output_x, output_y)
    }
}

/// Trackpads report a large absolute-coordinate delta even for a small patch
/// of finger movement.  Their transfer function is kept separate from mouse
/// counts: output magnitude is approximately `base*x + quadratic*x^2`.
#[derive(Clone, Copy, Debug)]
pub struct TrackpadSettings {
    pub speed: f32,
    pub base_gain: f32,
    pub quadratic_gain: f32,
    pub max_gain: f32,
    pub click_guard_distance: f32,
    pub click_guard_reports: u8,
    pub smoothing: f32,
}

impl Default for TrackpadSettings {
    fn default() -> Self {
        Self {
            speed: 5.0,
            base_gain: 0.06,
            quadratic_gain: 0.003,
            max_gain: 4.0,
            click_guard_distance: 8.0,
            click_guard_reports: 3,
            smoothing: 0.10,
        }
    }
}

impl TrackpadSettings {
    pub fn sanitized(mut self) -> Self {
        self.speed = self.speed.clamp(0.10, 8.0);
        self.base_gain = self.base_gain.clamp(0.0, 1.0);
        self.quadratic_gain = self.quadratic_gain.clamp(0.0, 0.05);
        self.max_gain = self.max_gain.clamp(0.25, 8.0);
        self.click_guard_distance = self.click_guard_distance.clamp(0.0, 64.0);
        self.click_guard_reports = self.click_guard_reports.min(8);
        self.smoothing = self.smoothing.clamp(0.0, 0.95);
        self
    }

    pub fn gain(&self, speed: f32) -> f32 {
        ((self.base_gain + self.quadratic_gain * speed.max(0.0)) * self.speed).min(self.max_gain)
    }
}

#[derive(Debug, Default)]
pub struct TrackpadFilter {
    fraction_x: f32,
    fraction_y: f32,
    filtered_speed: f32,
    buttons: u8,
    click_guard_remaining: u8,
}

impl TrackpadFilter {
    pub fn apply(
        &mut self,
        dx: i32,
        dy: i32,
        buttons: u8,
        settings: &TrackpadSettings,
    ) -> (i32, i32) {
        let settings = settings.sanitized();
        if buttons != self.buttons {
            self.buttons = buttons;
            self.click_guard_remaining = settings.click_guard_reports;
            self.fraction_x = 0.0;
            self.fraction_y = 0.0;
        }

        let raw_x = dx as f32;
        let raw_y = dy as f32;
        let abs_x = raw_x.abs();
        let abs_y = raw_y.abs();
        let (major, minor) = if abs_x >= abs_y {
            (abs_x, abs_y)
        } else {
            (abs_y, abs_x)
        };
        let instant_speed = major + minor * 0.375;

        if self.click_guard_remaining != 0 {
            self.click_guard_remaining -= 1;
            if instant_speed <= settings.click_guard_distance {
                self.filtered_speed *= 0.5;
                return (0, 0);
            }
        }
        if dx == 0 && dy == 0 {
            self.filtered_speed *= settings.smoothing;
            return (0, 0);
        }

        self.filtered_speed =
            self.filtered_speed * settings.smoothing + instant_speed * (1.0 - settings.smoothing);
        let gain = settings.gain(self.filtered_speed);
        let scaled_x = raw_x * gain + self.fraction_x;
        let scaled_y = raw_y * gain + self.fraction_y;
        let output_x = scaled_x as i32;
        let output_y = scaled_y as i32;
        self.fraction_x = scaled_x - output_x as f32;
        self.fraction_y = scaled_y - output_y as f32;
        (output_x, output_y)
    }
}

/// Breaks a low-rate trackpad report into short per-frame steps. This changes
/// only presentation cadence; the sum and direction remain exact.
#[derive(Debug, Default)]
pub struct TrackpadInterpolator {
    pending_x: i32,
    pending_y: i32,
    steps: u8,
}

// The boot loop ticks every 1 ms. Four slices are enough to hide the larger
// steps of an absolute-coordinate trackpad while keeping input-to-cursor lag
// below one display frame. A longer window builds a visible motion backlog
// when reports keep arriving.
const TRACKPAD_INTERPOLATION_STEPS: u8 = 4;

impl TrackpadInterpolator {
    pub fn enqueue(&mut self, dx: i32, dy: i32) {
        self.pending_x = self.pending_x.saturating_add(dx);
        self.pending_y = self.pending_y.saturating_add(dy);
        if self.pending_x != 0 || self.pending_y != 0 {
            self.steps = TRACKPAD_INTERPOLATION_STEPS;
        }
    }

    pub fn has_pending(&self) -> bool {
        self.steps != 0 && (self.pending_x != 0 || self.pending_y != 0)
    }

    pub fn take(&mut self) -> Option<(i32, i32)> {
        if self.steps == 0 || (self.pending_x == 0 && self.pending_y == 0) {
            self.steps = 0;
            return None;
        }
        let steps = self.steps as i32;
        let portion = |value: i32| {
            if value > 0 {
                (value + steps - 1) / steps
            } else if value < 0 {
                (value - steps + 1) / steps
            } else {
                0
            }
        };
        let dx = portion(self.pending_x);
        let dy = portion(self.pending_y);
        self.pending_x -= dx;
        self.pending_y -= dy;
        self.steps -= 1;
        if self.pending_x == 0 && self.pending_y == 0 {
            self.steps = 0;
        }
        Some((dx, dy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_curve_maps_sixteen_counts_to_about_three_pixels() {
        let settings = MotionSettings::default();
        let mut filter = MotionFilter::default();
        let slow = filter.apply(16, 0, 0, &settings).0;
        assert!((2..=3).contains(&slow));

        let mut fast_filter = MotionFilter::default();
        let fast = fast_filter.apply(100, 0, 0, &settings).0;
        assert!(fast >= 45);
        assert!(fast > slow * 10);
        assert!(settings.gain(10_000.0) <= settings.max_gain + 0.01);
    }

    #[test]
    fn alternating_noise_cancels_but_coherent_motion_accumulates() {
        let settings = MotionSettings::default();
        let mut filter = MotionFilter::default();
        assert_eq!(filter.apply(1, 0, 0, &settings), (0, 0));
        assert_eq!(filter.apply(-1, 0, 0, &settings), (0, 0));
        assert_eq!(filter.apply(1, 0, 0, &settings), (0, 0));
        let mut coherent_output = 0;
        for _ in 0..10 {
            coherent_output += filter.apply(1, 0, 0, &settings).0;
        }
        assert!(coherent_output >= 1);
    }

    #[test]
    fn click_guard_suppresses_jitter_but_allows_a_fast_drag() {
        let settings = MotionSettings::default();
        let mut filter = MotionFilter::default();
        assert_eq!(filter.apply(1, 0, 1, &settings), (0, 0));
        assert_eq!(filter.apply(-1, 0, 1, &settings), (0, 0));
        assert!(filter.apply(32, 0, 1, &settings).0 >= 7);
    }

    #[test]
    fn trackpad_curve_is_quadratic_and_strongly_reduces_small_motion() {
        let settings = TrackpadSettings::default();
        let mut filter = TrackpadFilter::default();
        let slow = filter.apply(25, 0, 0, &settings).0;
        assert!((13..=17).contains(&slow));

        let mut medium_filter = TrackpadFilter::default();
        let medium = medium_filter.apply(50, 0, 0, &settings).0;
        assert!((45..=55).contains(&medium));

        let mut fast_filter = TrackpadFilter::default();
        let fast = fast_filter.apply(100, 0, 0, &settings).0;
        assert!(fast >= 160);
        assert!(fast > medium * 3);
    }

    #[test]
    fn trackpad_interpolation_is_smooth_and_distance_exact() {
        let mut interpolation = TrackpadInterpolator::default();
        interpolation.enqueue(15, -7);
        let mut total = (0, 0);
        let mut frames = 0;
        while let Some((dx, dy)) = interpolation.take() {
            assert!(dx.abs() <= 4);
            assert!(dy.abs() <= 2);
            total.0 += dx;
            total.1 += dy;
            frames += 1;
        }
        assert_eq!(frames, TRACKPAD_INTERPOLATION_STEPS);
        assert_eq!(total, (15, -7));
    }

    #[test]
    fn continuous_trackpad_reports_do_not_build_a_long_backlog() {
        let mut interpolation = TrackpadInterpolator::default();
        interpolation.enqueue(40, 20);
        assert!(interpolation.take().is_some());

        // A new report may arrive before the previous four slices finish.
        // All outstanding distance must still catch up within four ticks.
        interpolation.enqueue(24, 12);
        let mut ticks = 0;
        while interpolation.take().is_some() {
            ticks += 1;
        }
        assert!(ticks <= TRACKPAD_INTERPOLATION_STEPS as usize);
        assert!(!interpolation.has_pending());
    }
}
