//! Whole-interface zoom (Herdr's `app.zoom_in/out/reset`): every size of the chrome (sidebar, bars, views, dialogs,
//! text) is written as `px(...)`; the `px` here multiplies it by the zoom, so one number scales them all. The terminal
//! text keeps its own zoom (Ctrl+= / Ctrl+- / Ctrl+0) and is not drawn with this `px`. Saved per device in
//! `native-ui-zoom.json` in the state folder.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gpui::Pixels;
use serde_json::{Value, json};

static ZOOM: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0

/// The sizes the zoom steps through.
pub const STEPS: [f32; 9] = [0.7, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

pub fn zoom() -> f32 {
    f32::from_bits(ZOOM.load(Ordering::Relaxed))
}

pub fn set(z: f32) {
    ZOOM.store(z.clamp(STEPS[0], STEPS[STEPS.len() - 1]).to_bits(), Ordering::Relaxed);
}

/// A size of the interface in points, scaled by the zoom.
pub fn px(v: f32) -> Pixels {
    gpui::px(v * zoom())
}

pub fn percent() -> u32 {
    (zoom() * 100.0).round() as u32
}

/// The next size: one step up or down from the nearest step, or 100 % when `by` is 0.
pub fn step(current: f32, by: i32) -> f32 {
    if by == 0 {
        return 1.0;
    }
    let nearest = STEPS.iter().enumerate()
        .min_by(|a, b| (a.1 - current).abs().total_cmp(&(b.1 - current).abs())).map(|(i, _)| i).unwrap_or(3);
    STEPS[(nearest as i32 + by.signum()).clamp(0, STEPS.len() as i32 - 1) as usize]
}

pub fn path() -> PathBuf {
    drover_proto::client::state_dir().join("native-ui-zoom.json")
}

pub fn parse(text: &str) -> f32 {
    serde_json::from_str::<Value>(text).ok().and_then(|v| v["zoom"].as_f64()).map(|z| z as f32)
        .filter(|z| (STEPS[0]..=STEPS[STEPS.len() - 1]).contains(z)).unwrap_or(1.0)
}

pub fn save_to(path: &Path, zoom: f32) -> Result<(), String> {
    if (zoom - 1.0).abs() < f32::EPSILON {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::write(path, json!({"zoom": zoom}).to_string()).map_err(|e| e.to_string())
}

/// Load this device's zoom before the first window is drawn.
pub fn init() {
    set(std::fs::read_to_string(path()).map(|t| parse(&t)).unwrap_or(1.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_go_up_and_down_and_stop_at_the_ends() {
        assert_eq!(step(1.0, 1), 1.1);
        assert_eq!(step(1.0, -1), 0.9);
        assert_eq!(step(2.0, 1), 2.0);
        assert_eq!(step(0.7, -1), 0.7);
        assert_eq!(step(1.5, 0), 1.0);
        assert_eq!(step(1.2, 1), 1.5); // between steps: from the nearest (1.25)
    }

    #[test]
    fn parses_only_sane_zooms() {
        assert_eq!(parse(r#"{"zoom":1.25}"#), 1.25);
        assert_eq!(parse(r#"{"zoom":9}"#), 1.0);
        assert_eq!(parse("nonsense"), 1.0);
    }

    #[test]
    fn saves_loads_and_clears() {
        let file = std::env::temp_dir().join(format!("drover-zoom-{}.json", std::process::id()));
        save_to(&file, 1.5).unwrap();
        assert_eq!(parse(&std::fs::read_to_string(&file).unwrap()), 1.5);
        save_to(&file, 1.0).unwrap();
        assert!(!file.exists());
        save_to(&file, 1.0).unwrap(); // already gone: not an error
    }
}
