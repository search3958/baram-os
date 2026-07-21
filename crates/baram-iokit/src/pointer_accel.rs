//! Stateful pointer acceleration and click-jitter suppression.
//!
//! Two rational curves create low-speed precision, nearly 1:1 ordinary
//! movement, and a steep but bounded high-speed ramp without hard knees.

#[derive(Clone, Copy, Debug)]
pub struct MotionSettings {
    /// Master multiplier. 1.0 preserves the default medium-speed feel.
    pub speed: f32,
    /// Strength of only the high-speed part of the curve.
    pub acceleration: f32,
    /// Gain approached by very slow motion.
    pub precision_gain: f32,
    pub precision_knee: f32,
    pub acceleration_knee: f32,
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
            acceleration: 1.0,
            precision_gain: 0.55,
            precision_knee: 2.0,
            acceleration_knee: 14.0,
            max_gain: 3.4,
            click_guard_distance: 2.25,
            click_guard_reports: 2,
            smoothing: 0.20,
        }
    }
}

impl MotionSettings {
    pub fn sanitized(mut self) -> Self {
        self.speed = self.speed.clamp(0.10, 4.0);
        self.acceleration = self.acceleration.clamp(0.0, 2.0);
        self.precision_gain = self.precision_gain.clamp(0.10, 1.0);
        self.precision_knee = self.precision_knee.clamp(0.25, 32.0);
        self.acceleration_knee = self.acceleration_knee.clamp(1.0, 128.0);
        self.max_gain = self.max_gain.clamp(1.0, 8.0);
        self.click_guard_distance = self.click_guard_distance.clamp(0.0, 8.0);
        self.click_guard_reports = self.click_guard_reports.min(8);
        self.smoothing = self.smoothing.clamp(0.0, 0.95);
        self
    }

    pub fn gain(&self, speed: f32) -> f32 {
        let speed = speed.max(0.0);
        let speed2 = speed * speed;
        let precision_knee2 = self.precision_knee * self.precision_knee;
        let precision_rise = speed2 / (speed2 + precision_knee2);
        let precision = self.precision_gain + (1.0 - self.precision_gain) * precision_rise;

        // Fourth-order rational ramp: flat through normal movement, then a
        // rapid rise whose inverse-power tail approaches max_gain smoothly.
        let speed4 = speed2 * speed2;
        let acceleration_knee2 = self.acceleration_knee * self.acceleration_knee;
        let acceleration_knee4 = acceleration_knee2 * acceleration_knee2;
        let fast_rise = speed4 / (speed4 + acceleration_knee4);
        let fast = (self.max_gain - 1.0) * self.acceleration * fast_rise;
        (precision + fast) * self.speed
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_has_precision_normal_and_fast_regions() {
        let settings = MotionSettings::default();
        assert!(settings.gain(1.0) < 0.75);
        assert!((settings.gain(5.0) - 1.0).abs() < 0.15);
        assert!(settings.gain(24.0) > 3.0);
        assert!(settings.gain(1000.0) <= settings.max_gain + 0.01);
    }

    #[test]
    fn alternating_noise_cancels_but_coherent_motion_accumulates() {
        let settings = MotionSettings::default();
        let mut filter = MotionFilter::default();
        assert_eq!(filter.apply(1, 0, 0, &settings), (0, 0));
        assert_eq!(filter.apply(-1, 0, 0, &settings), (0, 0));
        assert_eq!(filter.apply(1, 0, 0, &settings), (0, 0));
        assert_eq!(filter.apply(1, 0, 0, &settings).0, 1);
    }

    #[test]
    fn click_guard_suppresses_jitter_but_allows_a_fast_drag() {
        let settings = MotionSettings::default();
        let mut filter = MotionFilter::default();
        assert_eq!(filter.apply(1, 0, 1, &settings), (0, 0));
        assert_eq!(filter.apply(-1, 0, 1, &settings), (0, 0));
        assert!(filter.apply(8, 0, 1, &settings).0 >= 8);
    }
}
