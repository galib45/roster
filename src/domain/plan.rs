//! Generated roster plans: dated shifts, per-seat slots, and assignments.
//!
//! A [`ShiftTemplate`](super::ShiftTemplate) is a reusable rule. A
//! [`RosterPlan`] is one concrete schedule for a date range: each template
//! occurrence becomes a [`DatedShift`], and every required seat becomes a
//! [`ShiftSlot`]. Headcount 2 means two slots, not one slot with a count.
//!
//! Generated plans snapshot `name`, `start`, and `end` from the template at
//! generation time, so later template edits never rewrite an existing plan.
//! [`DatedShift::template_id`] remains as provenance only.

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::shift::duration_minutes;
use super::{DomainError, Roster, ShiftTemplate};

/// Lifecycle of a generated plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Created but not yet solved.
    Draft,
    /// The solver has filled what it can.
    Generated,
    /// Locked; assignments may no longer change.
    Finalized,
}

/// A read-only assignment carried over from the day before the plan starts.
///
/// The admin supplies context; the domain does not infer it from an existing
/// plan. It exists so Phase 3 can evaluate rest rules across the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssignment {
    /// The date the shift started (the day before the plan start).
    pub date: NaiveDate,
    pub name: String,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub employee_id: String,
    /// The roster template this shift came from, when known.
    ///
    /// Optional so older files stay valid. It lets rest and unavailability rules
    /// match previous-day context the same way they match dated shifts.
    #[serde(default)]
    pub template_id: Option<String>,
}

/// One fillable seat on a dated shift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftSlot {
    pub id: String,
    /// The organization-level position this seat requires.
    pub position_id: String,
    /// The assigned employee, or `None` while the seat is unfilled.
    pub employee_id: Option<String>,
}

impl ShiftSlot {
    /// Create an unfilled slot for a position.
    pub fn new(position_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            position_id: position_id.into(),
            employee_id: None,
        }
    }

    /// True when an employee occupies this seat.
    pub fn is_filled(&self) -> bool {
        self.employee_id.is_some()
    }
}

/// One concrete occurrence of a shift template on a date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatedShift {
    pub id: String,
    /// The template this occurrence came from. Provenance only.
    pub template_id: String,
    /// The date the shift starts.
    pub date: NaiveDate,
    /// Snapshot of the template name.
    pub name: String,
    /// Snapshot of the template start time.
    pub start: NaiveTime,
    /// Snapshot of the template end time.
    pub end: NaiveTime,
    /// One slot per required seat.
    pub slots: Vec<ShiftSlot>,
}

impl DatedShift {
    /// Instantiate a template on a date, producing one slot per required seat.
    pub fn from_template(template: &ShiftTemplate, date: NaiveDate) -> Self {
        let slots = template
            .required_positions
            .iter()
            .flat_map(|requirement| {
                (0..requirement.headcount).map(|_| ShiftSlot::new(requirement.position_id.clone()))
            })
            .collect();

        Self {
            id: Uuid::new_v4().to_string(),
            template_id: template.id.clone(),
            date,
            name: template.name.clone(),
            start: template.start,
            end: template.end,
            slots,
        }
    }

    /// Length of this occurrence in minutes, wrapping across midnight.
    pub fn duration_minutes(&self) -> u32 {
        duration_minutes(self.start, self.end)
    }

    /// True when this occurrence ends on the following calendar day.
    pub fn crosses_midnight(&self) -> bool {
        self.end < self.start
    }

    /// Find a slot by ID.
    pub fn slot(&self, id: &str) -> Option<&ShiftSlot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    /// Find a slot by ID for mutation. Crate-visible so `Organization` stays
    /// the validated entry point.
    pub(crate) fn slot_mut(&mut self, id: &str) -> Option<&mut ShiftSlot> {
        self.slots.iter_mut().find(|slot| slot.id == id)
    }
}

/// A generated roster for one date range.
///
/// A roster may keep several plans (for example November and December), each
/// with its own status and dated shifts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterPlan {
    pub id: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: PlanStatus,
    /// The day before `start_date`, supplied by the admin.
    #[serde(default)]
    pub context: Vec<ContextAssignment>,
    pub shifts: Vec<DatedShift>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RosterPlan {
    /// Expand a roster's templates into dated shifts for an inclusive range.
    ///
    /// Templates occur on the weekdays listed in
    /// [`ShiftTemplate::weekdays`](super::ShiftTemplate::weekdays); an empty
    /// list means every day. The plan starts in [`PlanStatus::Draft`] and
    /// every slot is unfilled.
    pub fn generate(
        roster: &Roster,
        start: NaiveDate,
        end: NaiveDate,
        context: Vec<ContextAssignment>,
    ) -> Result<Self, DomainError> {
        if end < start {
            return Err(DomainError::InvalidDateRange { start, end });
        }

        let mut shifts = Vec::new();
        let mut date = start;
        while date <= end {
            for template in &roster.shifts {
                if template.occurs_on(date.weekday()) {
                    shifts.push(DatedShift::from_template(template, date));
                }
            }

            match date.succ_opt() {
                Some(next) => date = next,
                None => break,
            }
        }

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            start_date: start,
            end_date: end,
            status: PlanStatus::Draft,
            context,
            shifts,
            created_at: now,
            updated_at: now,
        })
    }

    /// Find a dated shift by ID.
    pub fn dated_shift(&self, id: &str) -> Option<&DatedShift> {
        self.shifts.iter().find(|shift| shift.id == id)
    }

    /// Find a dated shift by ID for mutation.
    pub(crate) fn dated_shift_mut(&mut self, id: &str) -> Option<&mut DatedShift> {
        self.shifts.iter_mut().find(|shift| shift.id == id)
    }

    /// Find a slot by its dated-shift and slot IDs.
    pub fn slot(&self, dated_shift_id: &str, slot_id: &str) -> Option<&ShiftSlot> {
        self.dated_shift(dated_shift_id)?.slot(slot_id)
    }

    /// Total number of seats across every dated shift.
    pub fn total_slots(&self) -> usize {
        self.shifts.iter().map(|shift| shift.slots.len()).sum()
    }

    /// Number of seats that currently have an employee.
    pub fn filled_slots(&self) -> usize {
        self.shifts
            .iter()
            .flat_map(|shift| &shift.slots)
            .filter(|slot| slot.is_filled())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::StaffingRequirement;
    use chrono::Weekday;

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn november_roster() -> Roster {
        let mut roster = Roster::new("Doctors").unwrap();

        let mut opd = ShiftTemplate::new("OPD", at(8, 0), at(14, 0)).unwrap();
        opd.weekdays = vec![
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Sat,
            Weekday::Sun,
        ];
        opd.required_positions.push(StaffingRequirement {
            position_id: "medical-officer".to_owned(),
            headcount: 2,
        });

        let mut morning = ShiftTemplate::new("Emergency Morning", at(8, 0), at(15, 0)).unwrap();
        morning.required_positions.push(StaffingRequirement {
            position_id: "medical-officer".to_owned(),
            headcount: 1,
        });

        let mut night = ShiftTemplate::new("Emergency Night", at(15, 0), at(8, 0)).unwrap();
        night.required_positions.push(StaffingRequirement {
            position_id: "medical-officer".to_owned(),
            headcount: 1,
        });

        roster.push_shift(opd);
        roster.push_shift(morning);
        roster.push_shift(night);
        roster
    }

    #[test]
    fn generates_the_november_example() {
        let roster = november_roster();
        let plan =
            RosterPlan::generate(&roster, date(2026, 11, 16), date(2026, 11, 30), Vec::new())
                .unwrap();

        assert_eq!(plan.start_date, date(2026, 11, 16));
        assert_eq!(plan.end_date, date(2026, 11, 30));
        assert_eq!(plan.status, PlanStatus::Draft);
        assert_eq!(plan.shifts.len(), 43);
        assert_eq!(plan.total_slots(), 56);
        assert_eq!(plan.filled_slots(), 0);

        let fridays: Vec<NaiveDate> = plan
            .shifts
            .iter()
            .filter(|shift| shift.date == date(2026, 11, 20))
            .map(|shift| shift.date)
            .collect();
        assert!(
            plan.shifts
                .iter()
                .all(|shift| shift.name != "OPD" || shift.date != date(2026, 11, 20))
        );
        assert!(
            plan.shifts
                .iter()
                .all(|shift| shift.name != "OPD" || shift.date != date(2026, 11, 27))
        );
        assert_eq!(fridays.len(), 2, "Friday still has two emergency shifts");

        for shift in &plan.shifts {
            match shift.name.as_str() {
                "OPD" => assert_eq!(shift.slots.len(), 2),
                "Emergency Morning" | "Emergency Night" => assert_eq!(shift.slots.len(), 1),
                other => panic!("unexpected shift {other}"),
            }
        }

        let night = plan
            .shifts
            .iter()
            .find(|shift| shift.name == "Emergency Night")
            .expect("night shift");
        assert_eq!(night.duration_minutes(), 1020);
        assert!(night.crosses_midnight());
        assert_eq!(night.slots[0].position_id, "medical-officer");
        assert_eq!(night.slots[0].employee_id, None);
    }

    #[test]
    fn snapshots_template_fields() {
        let mut roster = november_roster();
        let plan =
            RosterPlan::generate(&roster, date(2026, 11, 16), date(2026, 11, 16), Vec::new())
                .unwrap();
        let snapshot = plan
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD shift")
            .clone();

        let opd = roster
            .shifts
            .iter_mut()
            .find(|shift| shift.name == "OPD")
            .expect("OPD template");
        opd.name = "Old OPD".to_owned();
        opd.start = at(9, 0);
        opd.weekdays.clear();

        let existing = plan.dated_shift(&snapshot.id).expect("snapshot");
        assert_eq!(existing.name, "OPD");
        assert_eq!(existing.start, at(8, 0));
        assert_eq!(existing.end, at(14, 0));
        assert_eq!(existing.slots, snapshot.slots);

        let regenerated =
            RosterPlan::generate(&roster, date(2026, 11, 16), date(2026, 11, 16), Vec::new())
                .unwrap();
        assert_eq!(regenerated.shifts.len(), 3);
        assert!(
            regenerated
                .shifts
                .iter()
                .any(|shift| shift.name == "Old OPD")
        );
    }

    #[test]
    fn rejects_reversed_date_ranges() {
        let roster = november_roster();
        let error =
            RosterPlan::generate(&roster, date(2026, 11, 30), date(2026, 11, 16), Vec::new())
                .unwrap_err();

        assert_eq!(
            error,
            DomainError::InvalidDateRange {
                start: date(2026, 11, 30),
                end: date(2026, 11, 16)
            }
        );
    }

    #[test]
    fn keeps_context_and_finds_slots() {
        let roster = november_roster();
        let context = ContextAssignment {
            date: date(2026, 11, 15),
            name: "Emergency Night".to_owned(),
            start: at(15, 0),
            end: at(8, 0),
            employee_id: "employee-1".to_owned(),
            template_id: None,
        };
        let plan = RosterPlan::generate(
            &roster,
            date(2026, 11, 16),
            date(2026, 11, 16),
            vec![context.clone()],
        )
        .unwrap();

        assert_eq!(plan.context, vec![context]);
        let dated = &plan.shifts[0];
        let slot = plan
            .slot(&dated.id, &dated.slots[0].id)
            .expect("slot lookup");
        assert_eq!(slot.id, dated.slots[0].id);
        assert!(plan.slot(&dated.id, "missing-slot").is_none());
        assert!(plan.slot("missing-shift", &dated.slots[0].id).is_none());
    }
}
