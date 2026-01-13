use serde::{Deserialize, Serialize};

use crate::game::PlayType;

/// Play settings (options selected before playing)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub style: Style,
    pub style2: Option<Style>, // For DP second side
    pub gauge: GaugeType,
    pub assist: AssistType,
    pub range: RangeType,
    pub flip: bool,
    pub battle: bool,
    pub h_ran: bool,
}

impl Settings {
    /// P2 settings offset (4 * 15 = 60 bytes)
    pub const P2_OFFSET: u64 = 60;
    pub const WORD_SIZE: u64 = 4;

    /// Build settings from raw memory values
    #[allow(clippy::too_many_arguments)] // Mapping raw memory layout requires many parameters
    pub fn from_raw_values(
        play_type: PlayType,
        style_val: i32,
        style2_val: i32,
        gauge_val: i32,
        assist_val: i32,
        range_val: i32,
        flip_val: i32,
        battle_val: i32,
        h_ran_val: i32,
    ) -> Self {
        Self {
            style: Style::from_i32(style_val),
            style2: if play_type == PlayType::Dp {
                Some(Style::from_i32(style2_val))
            } else {
                None
            },
            gauge: GaugeType::from_i32(gauge_val),
            assist: AssistType::from_i32(assist_val),
            range: RangeType::from_i32(range_val),
            flip: flip_val == 1,
            battle: battle_val == 1,
            h_ran: h_ran_val == 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Style {
    #[default]
    Off,
    Random,
    RRandom,
    SRandom,
    Mirror,
    SynchronizeRandom,
    SymmetryRandom,
}

impl Style {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::Random,
            2 => Self::RRandom,
            3 => Self::SRandom,
            4 => Self::Mirror,
            5 => Self::SynchronizeRandom,
            6 => Self::SymmetryRandom,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Random => "RANDOM",
            Self::RRandom => "R-RANDOM",
            Self::SRandom => "S-RANDOM",
            Self::Mirror => "MIRROR",
            Self::SynchronizeRandom => "SYNCHRONIZE RANDOM",
            Self::SymmetryRandom => "SYMMETRY RANDOM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GaugeType {
    #[default]
    Off,
    AssistEasy,
    Easy,
    Hard,
    ExHard,
}

impl GaugeType {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::AssistEasy,
            2 => Self::Easy,
            3 => Self::Hard,
            4 => Self::ExHard,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::AssistEasy => "ASSIST EASY",
            Self::Easy => "EASY",
            Self::Hard => "HARD",
            Self::ExHard => "EX HARD",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AssistType {
    #[default]
    Off,
    AutoScratch,
    FiveKeys,
    LegacyNote,
    KeyAssist,
    AnyKey,
}

impl AssistType {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::AutoScratch,
            2 => Self::FiveKeys,
            3 => Self::LegacyNote,
            4 => Self::KeyAssist,
            5 => Self::AnyKey,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::AutoScratch => "AUTO SCRATCH",
            Self::FiveKeys => "5KEYS",
            Self::LegacyNote => "LEGACY NOTE",
            Self::KeyAssist => "KEY ASSIST",
            Self::AnyKey => "ANY KEY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RangeType {
    #[default]
    Off,
    SuddenPlus,
    HiddenPlus,
    SudHid,
    Lift,
    LiftSud,
}

impl RangeType {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::SuddenPlus,
            2 => Self::HiddenPlus,
            3 => Self::SudHid,
            4 => Self::Lift,
            5 => Self::LiftSud,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::SuddenPlus => "SUDDEN+",
            Self::HiddenPlus => "HIDDEN+",
            Self::SudHid => "SUD+ & HID+",
            Self::Lift => "LIFT",
            Self::LiftSud => "LIFT & SUD+",
        }
    }
}
