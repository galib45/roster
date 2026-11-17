//! Shift templates: the coverage demand a roster has to satisfy.

use chrono::{NaiveTime, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// How many people a shift needs for one organization-level position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaffingRequirement {
    /// The organization-level position required.
    pub position_id: String,
    /// How many employees with that position the shift needs.
    pub headcount: u16,
}

/// One shift template owned by a roster.
///
/// Times are wall-clock times on the shift's start date. An end time earlier
/// than the start time means the shift crosses midnight (for example
/// 15:00–08:00 is a 17-hour night shift).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftTemplate {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    pub start: NaiveTime,
    pub end: NaiveTime,
    /// Positions and headcounts this shift must cover.
    #[serde(default)]
    pub required_positions: Vec<StaffingRequirement>,
    /// Weekdays this template occurs on. Empty means every day.
    #[serde(default)]
    pub weekdays: Vec<Weekday>,
}

impl ShiftTemplate {
    /// Create a shift template.
    ///
    /// The name is trimmed and blank names are rejected. Equal start and end
    /// times are rejected because they are ambiguous rather than a 24-hour
    /// shift; a full-day shift must be expressed with different times.
    pub fn new(name: &str, start: NaiveTime, end: NaiveTime) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }
        if start == end {
            return Err(DomainError::InvalidShiftTimes { start, end });
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            start,
            end,
            required_positions: Vec::new(),
            weekdays: Vec::new(),
        })
    }

    /// Length of the shift in minutes, wrapping across midnight.
    pub fn duration_minutes(&self) -> u32 {
        duration_minutes(self.start, self.end)
    }

    /// True when the shift ends on the following calendar day.
    pub fn crosses_midnight(&self) -> bool {
        self.end < self.start
    }

    /// True when this template occurs on the given weekday.
    pub fn occurs_on(&self, weekday: Weekday) -> bool {
        self.weekdays.is_empty() || self.weekdays.contains(&weekday)
    }
}

/// Minutes between two wall-clock times, wrapping across midnight.
///
/// Shared by shift templates and generated dated shifts so the overnight
/// formula exists in exactly one place.
pub(crate) fn duration_minutes(start: NaiveTime, end: NaiveTime) -> u32 {
    let start = start.hour() * 60 + start.minute();
    let end = end.hour() * 60 + end.minute();

    if end > start {
        end - start
    } else {
        end + 24 * 60 - start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    #[test]
    fn computes_daytime_durations() {
        let shift = ShiftTemplate::new("OPD", at(9, 0), at(17, 0)).unwrap();

        assert_eq!(shift.duration_minutes(), 480);
        assert!(!shift.crosses_midnight());
    }

    #[test]
    fn computes_overnight_durations() {
        let shift = ShiftTemplate::new("Emergency Night", at(15, 0), at(8, 0)).unwrap();

        assert_eq!(shift.duration_minutes(), 1020);
        assert!(shift.crosses_midnight());
    }

    #[test]
    fn rejects_equal_start_and_end_times() {
        let time = at(8, 0);
        assert_eq!(
            ShiftTemplate::new("Impossible", time, time),
            Err(DomainError::InvalidShiftTimes {
                start: time,
                end: time
            })
        );
    }

    #[test]
    fn trims_shift_names_and_rejects_blanks() {
        let shift = ShiftTemplate::new("  OPD  ", at(8, 0), at(14, 0)).unwrap();
        assert_eq!(shift.name, "OPD");
        assert_eq!(
            ShiftTemplate::new("   ", at(8, 0), at(14, 0)),
            Err(DomainError::BlankName)
        );
    }

    #[test]
    fn empty_weekdays_mean_every_day() {
        let shift = ShiftTemplate::new("Emergency Night", at(15, 0), at(8, 0)).unwrap();

        for weekday in [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ] {
            assert!(shift.occurs_on(weekday));
        }
    }

    #[test]
    fn restricted_weekdays_only_occur_on_those_days() {
        let mut shift = ShiftTemplate::new("OPD", at(8, 0), at(14, 0)).unwrap();
        shift.weekdays = vec![
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Sat,
            Weekday::Sun,
        ];

        assert!(shift.occurs_on(Weekday::Mon));
        assert!(!shift.occurs_on(Weekday::Fri));
    }
}
