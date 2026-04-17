use chrono::{DateTime, Duration, Utc};

use crate::domain::{MacroEvent, MacroEventClass};

pub fn active(timestamp: DateTime<Utc>, events: &[MacroEvent]) -> bool {
    events.iter().any(|event| {
        let (before, after) = match event.class {
            MacroEventClass::FomcRateDecision | MacroEventClass::PowellPressConference => {
                (Duration::minutes(30), Duration::minutes(240))
            }
            MacroEventClass::Cpi
            | MacroEventClass::CoreCpi
            | MacroEventClass::Ppi
            | MacroEventClass::Nfp
            | MacroEventClass::UnemploymentRate
            | MacroEventClass::CorePce
            | MacroEventClass::GdpAdvance => (Duration::minutes(15), Duration::minutes(60)),
        };

        timestamp >= event.event_time - before && timestamp <= event.event_time + after
    })
}
