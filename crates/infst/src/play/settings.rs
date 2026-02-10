use serde::{Deserialize, Serialize};
use strum::{Display, IntoStaticStr};
use thiserror::Error;
use tracing::warn;

use crate::play::PlayType;

/// Error for invalid enum value conversion
#[derive(Debug, Error)]
#[error("Invalid {type_name} value: {value}")]
pub struct InvalidEnumValueError {
    type_name: &'static str,
    value: i32,
}

impl InvalidEnumValueError {
    pub fn new(type_name: &'static str, value: i32) -> Self {
        Self { type_name, value }
    }
}

/// Play settings (options selected before playing)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub style: Style,
    pub style2: Option<Style>, // For DP second side
    pub assist: AssistType,
    pub range: RangeType,
    pub flip: bool,
    pub battle: bool,
    pub h_ran: bool,
}

/// Raw settings values read directly from memory
#[derive(Debug, Clone, Default)]
pub struct RawSettings {
    pub play_type: PlayType,
    pub style: i32,
    pub style2: i32,
    pub assist: i32,
    pub range: i32,
    pub flip: i32,
    pub battle: i32,
    pub h_ran: i32,
}

impl Settings {
    /// P2 settings offset (4 * 15 = 60 bytes)
    pub const P2_OFFSET: u64 = 60;
    pub const WORD_SIZE: u64 = 4;

    /// Build settings from raw memory values.
    ///
    /// Invalid enum values are replaced with defaults and logged as warnings.
    /// This can occur when memory contains unexpected values during state transitions.
    pub fn from_raw(raw: RawSettings) -> Self {
        let style = raw.style.try_into().unwrap_or_else(|_| {
            warn!("Invalid style value: {}, using default", raw.style);
            Style::default()
        });

        let style2 = if raw.play_type == PlayType::Dp {
            Some(raw.style2.try_into().unwrap_or_else(|_| {
                warn!("Invalid style2 value: {}, using default", raw.style2);
                Style::default()
            }))
        } else {
            None
        };

        let assist = raw.assist.try_into().unwrap_or_else(|_| {
            warn!("Invalid assist value: {}, using default", raw.assist);
            AssistType::default()
        });

        let range = raw.range.try_into().unwrap_or_else(|_| {
            warn!("Invalid range value: {}, using default", raw.range);
            RangeType::default()
        });

        Self {
            style,
            style2,
            assist,
            range,
            flip: raw.flip == 1,
            battle: raw.battle == 1,
            h_ran: raw.h_ran == 1,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, IntoStaticStr, Display,
)]
#[repr(i32)]
pub enum Style {
    #[default]
    #[strum(serialize = "OFF")]
    Off = 0,
    #[strum(serialize = "RANDOM")]
    Random = 1,
    #[strum(serialize = "R-RANDOM")]
    RRandom = 2,
    #[strum(serialize = "S-RANDOM")]
    SRandom = 3,
    #[strum(serialize = "MIRROR")]
    Mirror = 4,
    #[strum(serialize = "SYNCHRONIZE RANDOM")]
    SynchronizeRandom = 5,
    #[strum(serialize = "SYMMETRY RANDOM")]
    SymmetryRandom = 6,
}

impl TryFrom<i32> for Style {
    type Error = InvalidEnumValueError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::Random),
            2 => Ok(Self::RRandom),
            3 => Ok(Self::SRandom),
            4 => Ok(Self::Mirror),
            5 => Ok(Self::SynchronizeRandom),
            6 => Ok(Self::SymmetryRandom),
            _ => Err(InvalidEnumValueError::new("Style", value)),
        }
    }
}

impl Style {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, IntoStaticStr, Display,
)]
#[repr(i32)]
pub enum AssistType {
    #[default]
    #[strum(serialize = "OFF")]
    Off = 0,
    #[strum(serialize = "AUTO SCRATCH")]
    AutoScratch = 1,
    #[strum(serialize = "5KEYS")]
    FiveKeys = 2,
    #[strum(serialize = "LEGACY NOTE")]
    LegacyNote = 3,
    #[strum(serialize = "KEY ASSIST")]
    KeyAssist = 4,
    #[strum(serialize = "ANY KEY")]
    AnyKey = 5,
}

impl TryFrom<i32> for AssistType {
    type Error = InvalidEnumValueError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::AutoScratch),
            2 => Ok(Self::FiveKeys),
            3 => Ok(Self::LegacyNote),
            4 => Ok(Self::KeyAssist),
            5 => Ok(Self::AnyKey),
            _ => Err(InvalidEnumValueError::new("AssistType", value)),
        }
    }
}

impl AssistType {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, IntoStaticStr, Display,
)]
#[repr(i32)]
pub enum RangeType {
    #[default]
    #[strum(serialize = "OFF")]
    Off = 0,
    #[strum(serialize = "SUDDEN+")]
    SuddenPlus = 1,
    #[strum(serialize = "HIDDEN+")]
    HiddenPlus = 2,
    #[strum(serialize = "SUD+ & HID+")]
    SudHid = 3,
    #[strum(serialize = "LIFT")]
    Lift = 4,
    #[strum(serialize = "LIFT & SUD+")]
    LiftSud = 5,
}

impl TryFrom<i32> for RangeType {
    type Error = InvalidEnumValueError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::SuddenPlus),
            2 => Ok(Self::HiddenPlus),
            3 => Ok(Self::SudHid),
            4 => Ok(Self::Lift),
            5 => Ok(Self::LiftSud),
            _ => Err(InvalidEnumValueError::new("RangeType", value)),
        }
    }
}

impl RangeType {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_try_from_valid() {
        assert_eq!(Style::try_from(0).unwrap(), Style::Off);
        assert_eq!(Style::try_from(1).unwrap(), Style::Random);
        assert_eq!(Style::try_from(4).unwrap(), Style::Mirror);
        assert_eq!(Style::try_from(6).unwrap(), Style::SymmetryRandom);
    }

    #[test]
    fn test_style_try_from_invalid() {
        assert!(Style::try_from(7).is_err());
        assert!(Style::try_from(-1).is_err());
        assert!(Style::try_from(100).is_err());
    }

    #[test]
    fn test_assist_type_try_from_valid() {
        assert_eq!(AssistType::try_from(0).unwrap(), AssistType::Off);
        assert_eq!(AssistType::try_from(1).unwrap(), AssistType::AutoScratch);
        assert_eq!(AssistType::try_from(5).unwrap(), AssistType::AnyKey);
    }

    #[test]
    fn test_assist_type_try_from_invalid() {
        assert!(AssistType::try_from(6).is_err());
        assert!(AssistType::try_from(-1).is_err());
    }

    #[test]
    fn test_range_type_try_from_valid() {
        assert_eq!(RangeType::try_from(0).unwrap(), RangeType::Off);
        assert_eq!(RangeType::try_from(1).unwrap(), RangeType::SuddenPlus);
        assert_eq!(RangeType::try_from(5).unwrap(), RangeType::LiftSud);
    }

    #[test]
    fn test_range_type_try_from_invalid() {
        assert!(RangeType::try_from(6).is_err());
        assert!(RangeType::try_from(-1).is_err());
    }

    #[test]
    fn test_settings_from_raw_p1() {
        let settings = Settings::from_raw(RawSettings {
            play_type: PlayType::P1,
            style: 1,
            style2: 0,
            assist: 0,
            range: 1,
            flip: 0,
            battle: 0,
            h_ran: 0,
        });
        assert_eq!(settings.style, Style::Random);
        assert!(settings.style2.is_none());
        assert_eq!(settings.range, RangeType::SuddenPlus);
        assert!(!settings.flip);
        assert!(!settings.battle);
        assert!(!settings.h_ran);
    }

    #[test]
    fn test_settings_from_raw_dp() {
        let settings = Settings::from_raw(RawSettings {
            play_type: PlayType::Dp,
            style: 4,
            style2: 1,
            assist: 0,
            range: 0,
            flip: 1,
            battle: 1,
            h_ran: 1,
        });
        assert_eq!(settings.style, Style::Mirror);
        assert_eq!(settings.style2, Some(Style::Random));
        assert!(settings.flip);
        assert!(settings.battle);
        assert!(settings.h_ran);
    }

    #[test]
    fn test_settings_from_raw_invalid_defaults() {
        // Invalid values should default to Off
        let settings = Settings::from_raw(RawSettings {
            play_type: PlayType::P1,
            style: 100,
            style2: 0,
            assist: 100,
            range: 100,
            flip: 0,
            battle: 0,
            h_ran: 0,
        });
        assert_eq!(settings.style, Style::Off);
        assert_eq!(settings.assist, AssistType::Off);
        assert_eq!(settings.range, RangeType::Off);
    }

    #[test]
    fn test_invalid_enum_value_error_display() {
        let err = InvalidEnumValueError::new("TestEnum", 42);
        assert_eq!(format!("{}", err), "Invalid TestEnum value: 42");
    }
}
