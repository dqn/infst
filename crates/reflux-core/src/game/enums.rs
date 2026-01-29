use serde::{Deserialize, Serialize};
use strum::{Display, EnumString, FromRepr, IntoStaticStr};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    FromRepr,
    EnumString,
    IntoStaticStr,
    Display,
)]
#[repr(u8)]
pub enum Difficulty {
    #[strum(serialize = "SPB")]
    SpB = 0,
    #[strum(serialize = "SPN")]
    SpN = 1,
    #[strum(serialize = "SPH")]
    SpH = 2,
    #[strum(serialize = "SPA")]
    SpA = 3,
    #[strum(serialize = "SPL")]
    SpL = 4,
    #[strum(serialize = "DPB")]
    DpB = 5,
    #[strum(serialize = "DPN")]
    DpN = 6,
    #[strum(serialize = "DPH")]
    DpH = 7,
    #[strum(serialize = "DPA")]
    DpA = 8,
    #[strum(serialize = "DPL")]
    DpL = 9,
}

impl Difficulty {
    pub fn from_u8(value: u8) -> Option<Self> {
        Self::from_repr(value)
    }

    pub fn is_sp(&self) -> bool {
        matches!(
            self,
            Self::SpB | Self::SpN | Self::SpH | Self::SpA | Self::SpL
        )
    }

    pub fn is_dp(&self) -> bool {
        !self.is_sp()
    }

    pub fn short_name(&self) -> &'static str {
        self.into()
    }

    /// Get the expanded difficulty name (e.g., "NORMAL", "HYPER")
    pub fn expand_name(&self) -> &'static str {
        match self {
            Self::SpB | Self::DpB => "BEGINNER",
            Self::SpN | Self::DpN => "NORMAL",
            Self::SpH | Self::DpH => "HYPER",
            Self::SpA | Self::DpA => "ANOTHER",
            Self::SpL | Self::DpL => "LEGGENDARIA",
        }
    }

    /// Get the color code for difficulty (for OBS output)
    pub fn color_code(&self) -> &'static str {
        match self {
            Self::SpB | Self::DpB => "#32CD32", // Green for beginner
            Self::SpN | Self::DpN => "#0FABFD", // Blue for normal
            Self::SpH | Self::DpH => "#F4903C", // Orange for hyper
            Self::SpA | Self::DpA => "#E52B19", // Red for another
            Self::SpL | Self::DpL => "#9B30FF", // Purple for leggendaria
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
    FromRepr,
    IntoStaticStr,
    Display,
)]
#[repr(u8)]
pub enum Lamp {
    #[default]
    #[strum(serialize = "NO PLAY")]
    NoPlay = 0,
    #[strum(serialize = "FAILED")]
    Failed = 1,
    #[strum(serialize = "ASSIST")]
    AssistClear = 2,
    #[strum(serialize = "EASY")]
    EasyClear = 3,
    #[strum(serialize = "CLEAR")]
    Clear = 4,
    #[strum(serialize = "HARD")]
    HardClear = 5,
    #[strum(serialize = "EX HARD")]
    ExHardClear = 6,
    #[strum(serialize = "FC")]
    FullCombo = 7,
    #[strum(serialize = "PFC")]
    Pfc = 8,
}

impl Lamp {
    pub fn from_u8(value: u8) -> Option<Self> {
        Self::from_repr(value)
    }

    pub fn short_name(&self) -> &'static str {
        self.into()
    }

    /// Get the expanded lamp name (for display and export)
    pub fn expand_name(&self) -> &'static str {
        match self {
            Self::NoPlay => "NO PLAY",
            Self::Failed => "FAILED",
            Self::AssistClear => "ASSIST CLEAR",
            Self::EasyClear => "EASY CLEAR",
            Self::Clear => "CLEAR",
            Self::HardClear => "HARD CLEAR",
            Self::ExHardClear => "EX HARD CLEAR",
            Self::FullCombo | Self::Pfc => "FULL COMBO",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Default,
    FromRepr,
    IntoStaticStr,
    Display,
)]
#[repr(u8)]
pub enum Grade {
    #[default]
    #[strum(serialize = "-")]
    NoPlay = 0,
    F = 1,
    E = 2,
    D = 3,
    C = 4,
    B = 5,
    A = 6,
    #[strum(serialize = "AA")]
    Aa = 7,
    #[strum(serialize = "AAA")]
    Aaa = 8,
}

impl Grade {
    pub fn from_u8(value: u8) -> Option<Self> {
        Self::from_repr(value)
    }

    pub fn from_score_ratio(ratio: f64) -> Self {
        if ratio >= 8.0 / 9.0 {
            Self::Aaa
        } else if ratio >= 7.0 / 9.0 {
            Self::Aa
        } else if ratio >= 6.0 / 9.0 {
            Self::A
        } else if ratio >= 5.0 / 9.0 {
            Self::B
        } else if ratio >= 4.0 / 9.0 {
            Self::C
        } else if ratio >= 3.0 / 9.0 {
            Self::D
        } else if ratio >= 2.0 / 9.0 {
            Self::E
        } else {
            Self::F
        }
    }

    pub fn short_name(&self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, IntoStaticStr, Display,
)]
pub enum PlayType {
    #[default]
    #[strum(serialize = "1P")]
    P1,
    #[strum(serialize = "2P")]
    P2,
    #[strum(serialize = "DP")]
    Dp,
}

impl PlayType {
    pub fn short_name(&self) -> &'static str {
        self.into()
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Default,
    FromRepr,
    IntoStaticStr,
    Display,
)]
#[repr(u8)]
pub enum UnlockType {
    #[default]
    Base = 0,
    Bits = 1,
    Sub = 2,
}

impl UnlockType {
    pub fn from_u8(value: u8) -> Option<Self> {
        Self::from_repr(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, IntoStaticStr, Display)]
pub enum GameState {
    #[default]
    Unknown,
    SongSelect,
    Playing,
    ResultScreen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_from_u8() {
        assert_eq!(Difficulty::from_u8(0), Some(Difficulty::SpB));
        assert_eq!(Difficulty::from_u8(4), Some(Difficulty::SpL));
        assert_eq!(Difficulty::from_u8(5), Some(Difficulty::DpB));
        assert_eq!(Difficulty::from_u8(9), Some(Difficulty::DpL));
        assert_eq!(Difficulty::from_u8(10), None);
    }

    #[test]
    fn test_difficulty_is_sp_dp() {
        assert!(Difficulty::SpN.is_sp());
        assert!(!Difficulty::SpN.is_dp());
        assert!(Difficulty::DpA.is_dp());
        assert!(!Difficulty::DpA.is_sp());
    }

    #[test]
    fn test_grade_from_score_ratio() {
        assert_eq!(Grade::from_score_ratio(1.0), Grade::Aaa);
        assert_eq!(Grade::from_score_ratio(0.9), Grade::Aaa);
        assert_eq!(Grade::from_score_ratio(8.0 / 9.0), Grade::Aaa);
        assert_eq!(Grade::from_score_ratio(7.0 / 9.0), Grade::Aa);
        assert_eq!(Grade::from_score_ratio(6.0 / 9.0), Grade::A);
        assert_eq!(Grade::from_score_ratio(5.0 / 9.0), Grade::B);
        assert_eq!(Grade::from_score_ratio(4.0 / 9.0), Grade::C);
        assert_eq!(Grade::from_score_ratio(3.0 / 9.0), Grade::D);
        assert_eq!(Grade::from_score_ratio(2.0 / 9.0), Grade::E);
        assert_eq!(Grade::from_score_ratio(0.1), Grade::F);
    }

    #[test]
    fn test_lamp_ordering() {
        assert!(Lamp::Pfc > Lamp::FullCombo);
        assert!(Lamp::FullCombo > Lamp::ExHardClear);
        assert!(Lamp::Failed < Lamp::Clear);
    }
}
