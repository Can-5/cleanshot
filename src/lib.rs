//! CleanShot: screenshot-domain logic.
//!
//! Owns the data model around screenshots — capture regions, annotations and archive
//! naming — so it can be unit-tested without a display server. The actual capture
//! (portal/S screenshot API / platform backend) is a platform capability that lives in
//! the Orbit host and is gated behind the `screen_capture` grant. This crate never
//! touches the screen.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum CleanShotError {
    #[error("region {0:?} has zero or negative extent")]
    BadRegion(Region),
    #[error("annotation text is {0} bytes; limit is {MAX_TEXT_BYTES}")]
    TextTooLong(usize),
    #[error("annotation bounds {0:?} escape the target {1:?}")]
    OutOfBounds(Region, (u32, u32)),
    #[error("blur radius must be >= {MIN_BLUR}, got {0}")]
    BlurTooSmall(u32),
}

pub const MAX_TEXT_BYTES: usize = 500;
pub const MIN_BLUR: u32 = 4;

/// Rectangle in image pixels, origin top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn validated(&self) -> Result<Self, CleanShotError> {
        if self.width == 0 || self.height == 0 {
            return Err(CleanShotError::BadRegion(*self));
        }
        Ok(*self)
    }

    pub fn right(&self) -> i64 {
        self.x as i64 + self.width as i64
    }

    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.height as i64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Annotation {
    Arrow { from: (i32, i32), to: (i32, i32), color: String, stroke: u8 },
    Rectangle { rect: Region, color: String, stroke: u8 },
    Text { at: (i32, i32), text: String, color: String, size: u16 },
    /// Pixelate/blur a secret region (passwords, tokens) before the image leaves memory.
    BlurRegion { rect: Region, radius: u32 },
    Highlight { rect: Region, color: String },
}

/// Validate an annotation against the target image size. Returns an error, never a
/// clamped value — an out-of-bounds annotation is a bug in the caller.
pub fn validate_annotation(annotation: &Annotation, image_size: (u32, u32)) -> Result<(), CleanShotError> {
    let (iw, ih) = image_size;
    let check_region = |r: &Region| -> Result<(), CleanShotError> {
        r.validated()?;
        if r.x < 0 || r.y < 0 || r.right() > iw as i64 || r.bottom() > ih as i64 {
            return Err(CleanShotError::OutOfBounds(*r, image_size));
        }
        Ok(())
    };
    let check_point = |p: (i32, i32)| -> Result<(), CleanShotError> {
        if p.0 < 0 || p.1 < 0 || p.0 >= iw as i32 || p.1 >= ih as i32 {
            return Err(CleanShotError::OutOfBounds(Region::new(p.0, p.1, 1, 1), image_size));
        }
        Ok(())
    };
    match annotation {
        Annotation::Arrow { from, to, .. } => {
            check_point(*from)?;
            check_point(*to)
        }
        Annotation::Rectangle { rect, .. } | Annotation::Highlight { rect, .. } => check_region(rect),
        Annotation::Text { at, text, .. } => {
            check_point(*at)?;
            if text.len() > MAX_TEXT_BYTES {
                return Err(CleanShotError::TextTooLong(text.len()));
            }
            Ok(())
        }
        Annotation::BlurRegion { rect, radius } => {
            check_region(rect)?;
            if *radius < MIN_BLUR {
                return Err(CleanShotError::BlurTooSmall(*radius));
            }
            Ok(())
        }
    }
}

/// Metadata a capture carries once the platform produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureMeta {
    pub width: u32,
    pub height: u32,
    /// Monotonic id supplied by the host (never a randomness-required field).
    pub sequence: u32,
    pub taken_at_ms: u64,
    /// Where Orbit will archive the file once the user confirms (or auto-save is on).
    pub suggested_name: String,
}

/// Deterministic archive filename: orbit-shot-YYYYMMDD-HHMMSS-NNN.png
pub fn archive_filename(taken_at_ms: u64, sequence: u32) -> String {
    let (y, m, d, hh, mm, ss) = civil_utc(taken_at_ms);
    format!("orbit-shot-{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}-{sequence:03}.png")
}

/// (year, month, day, hour, minute, second) for UTC millisecond timestamps.
fn civil_utc(ms: u64) -> (i64, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    (y, m, d, (rem / 3600) as u32, ((rem / 60) % 60) as u32, (rem % 60) as u32)
}

/// Howard Hinnant's civil-from-days (same algorithm as renamex; kept local on purpose —
/// Orbit modules share no code beyond serde).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_size_region_rejected() {
        assert!(Region::new(0, 0, 0, 10).validated().is_err());
        assert!(matches!(Region::new(0, 0, 10, 10).validated(), Ok(_)));
    }

    #[test]
    fn annotation_within_bounds_ok() {
        let a = Annotation::Rectangle { rect: Region::new(0, 0, 100, 100), color: "#fff".into(), stroke: 2 };
        assert!(validate_annotation(&a, (1920, 1080)).is_ok());
    }

    #[test]
    fn annotation_escapes_image_rejected() {
        let a = Annotation::Rectangle { rect: Region::new(1900, 0, 100, 100), color: "#fff".into(), stroke: 2 };
        assert!(matches!(validate_annotation(&a, (1920, 1080)), Err(CleanShotError::OutOfBounds(..))));
    }

    #[test]
    fn text_size_limit_enforced() {
        let a = Annotation::Text { at: (0, 0), text: "x".repeat(501), color: "#fff".into(), size: 12 };
        assert!(matches!(validate_annotation(&a, (100, 100)), Err(CleanShotError::TextTooLong(501))));
    }

    #[test]
    fn blur_radius_floor_enforced() {
        let a = Annotation::BlurRegion { rect: Region::new(0, 0, 50, 50), radius: 2 };
        assert!(matches!(validate_annotation(&a, (100, 100)), Err(CleanShotError::BlurTooSmall(2))));
        let ok = Annotation::BlurRegion { rect: Region::new(0, 0, 50, 50), radius: 8 };
        assert!(validate_annotation(&ok, (100, 100)).is_ok());
    }

    #[test]
    fn archive_filename_format() {
        // 946_684_800 s = 2000-01-01 00:00:00 UTC, seq 7
        let name = archive_filename(946_684_800_000, 7);
        assert_eq!(name, "orbit-shot-20000101-000000-007.png");
    }

    #[test]
    fn epoch_zero_archive_filename() {
        assert_eq!(archive_filename(0, 1), "orbit-shot-19700101-000000-001.png");
    }

    #[test]
    fn annotation_serializes_with_kind_tag() {
        let a = Annotation::Arrow { from: (0, 0), to: (10, 10), color: "#fff".into(), stroke: 2 };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"kind\":\"arrow\""));
    }
}
