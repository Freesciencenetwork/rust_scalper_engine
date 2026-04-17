use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candle {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub close_time: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub buy_volume: Option<f64>,
    pub sell_volume: Option<f64>,
    pub delta: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolFilters {
    pub tick_size: f64,
    pub lot_step: f64,
}

impl Candle {
    pub fn inferred_delta(&self) -> Option<f64> {
        self.delta
            .or_else(|| match (self.buy_volume, self.sell_volume) {
                (Some(buy), Some(sell)) => Some(buy - sell),
                _ => None,
            })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MacroEventClass {
    Cpi,
    CoreCpi,
    Ppi,
    Nfp,
    UnemploymentRate,
    CorePce,
    GdpAdvance,
    FomcRateDecision,
    PowellPressConference,
}

impl MacroEventClass {
    pub fn from_code(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Cpi),
            2 => Some(Self::CoreCpi),
            3 => Some(Self::Ppi),
            4 => Some(Self::Nfp),
            5 => Some(Self::UnemploymentRate),
            6 => Some(Self::CorePce),
            7 => Some(Self::GdpAdvance),
            8 => Some(Self::FomcRateDecision),
            9 => Some(Self::PowellPressConference),
            _ => None,
        }
    }

    pub fn code(self) -> u16 {
        match self {
            Self::Cpi => 1,
            Self::CoreCpi => 2,
            Self::Ppi => 3,
            Self::Nfp => 4,
            Self::UnemploymentRate => 5,
            Self::CorePce => 6,
            Self::GdpAdvance => 7,
            Self::FomcRateDecision => 8,
            Self::PowellPressConference => 9,
        }
    }
}

impl Serialize for MacroEventClass {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.code())
    }
}

impl<'de> Deserialize<'de> for MacroEventClass {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::from_code(value).ok_or_else(|| {
            serde::de::Error::custom(format!("unsupported macro event code {value}"))
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroEvent {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub event_time: DateTime<Utc>,
    pub class: MacroEventClass,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VolatilityRegime {
    Normal,
    High,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SystemMode {
    Active,
    Halted,
}

#[cfg(test)]
mod tests {
    use super::MacroEventClass;

    #[test]
    fn macro_event_class_serializes_to_numeric_codes() {
        let value = serde_json::to_string(&MacroEventClass::PowellPressConference)
            .expect("serialize macro event");
        assert_eq!(value, "9");
    }

    #[test]
    fn macro_event_class_deserializes_known_codes() {
        let known: MacroEventClass = serde_json::from_str("2").expect("deserialize known class");
        assert_eq!(known, MacroEventClass::CoreCpi);
    }

    #[test]
    fn macro_event_class_rejects_unknown_codes() {
        let unknown: Result<MacroEventClass, _> = serde_json::from_str("99");
        assert!(unknown.is_err());
    }
}
