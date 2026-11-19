//! Dated rosters: concrete schedules with dated shifts, per-seat slots, and
//! assignments.
//!
//! A [`ShiftTemplate`](super::ShiftTemplate) is a reusable rule owned by a
//! [`RosterTemplate`]. A [`Roster`] is one concrete schedule for a date range:
//! each template occurrence becomes a [`DatedShift`], and every required seat
//! becomes a [`ShiftSlot`]. Headcount 2 means two slots, not one slot with a
//! count.
//!
//! Creating a roster snapshots its member employees and dated shifts, so later
//! template or staffing edits never rewrite an existing roster.
//! [`DatedShift::template_id`] remains as provenance only.

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::shift::duration_minutes;
use super::{DomainError, RosterTemplate, ShiftTemplate};

/// Lifecycle of a dated roster.
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

/// A member snapshotted onto a dated roster.
///
/// The position is captured at creation time so later employee changes do not
/// rewrite which seats the member is eligible for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterMember {
    pub employee_id: String,
    pub position_id: String,
}

impl RosterMember {
    pub fn new(employee_id: impl Into<String>, position_id: impl Into<String>) -> Self {
        Self {
            employee_id: employee_id.into(),
            position_id: position_id.into(),
        }
    }
}

/// A read-only assignment carried over from the day before the roster starts.
///
/// The admin supplies context; the domain does not infer it from an existing
/// roster. It exists so Phase 3 can evaluate rest rules across the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssignment {
    /// The date the shift started (the day before the roster start).
    pub date: NaiveDate,
    pub name: String,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub employee_id: String,
    /// The shift template this shift came from, when known.
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

/// A dated roster for one inclusive date range.
///
/// A template may keep several rosters (for example November and December),
/// each with its own status, snapshot members, context, and dated shifts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    pub id: String,
    /// The template this roster was created from. Provenance only.
    pub template_id: String,
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub status: PlanStatus,
    /// Employees snapshotted at creation, with their positions.
    #[serde(default)]
    pub members: Vec<RosterMember>,
    /// The day before `start`, supplied by the admin.
    #[serde(default)]
    pub context: Vec<ContextAssignment>,
    pub shifts: Vec<DatedShift>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Roster {
    /// Create a dated roster from a template for an inclusive range.
    ///
    /// Templates occur on the weekdays listed in
    /// [`ShiftTemplate::weekdays`](super::ShiftTemplate::weekdays); an empty
    /// list means every day. The roster starts in [`PlanStatus::Draft`] and
    /// every slot is unfilled.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        template: &RosterTemplate,
        name: impl Into<String>,
        start: NaiveDate,
        end: NaiveDate,
        members: Vec<RosterMember>,
        context: Vec<ContextAssignment>,
    ) -> Result<Self, DomainError> {
        if end < start {
            return Err(DomainError::InvalidDateRange { start, end });
        }

        let mut shifts = Vec::new();
        let mut date = start;
        while date <= end {
            for shift in &template.shifts {
                if shift.occurs_on(date.weekday()) {
                    shifts.push(DatedShift::from_template(shift, date));
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
            template_id: template.id.clone(),
            name: name.into(),
            start,
            end,
            status: PlanStatus::Draft,
            members,
            context,
            shifts,
            created_at: now,
            updated_at: now,
        })
    }

    /// Find a snapshot member by employee ID.
    pub fn member(&self, employee_id: &str) -> Option<&RosterMember> {
        self.members
            .iter()
            .find(|member| member.employee_id == employee_id)
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

    fn november_template() -> RosterTemplate {
        let mut roster = RosterTemplate::new("Doctors").unwrap();
        roster.position_ids.push("medical-officer".to_owned());

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

    fn members() -> Vec<RosterMember> {
        vec![RosterMember::new("employee-1", "medical-officer")]
    }

    #[test]
    fn creates_the_november_example() {
        let template = november_template();
        let roster = Roster::create(
            &template,
            "Doctors — November",
            date(2026, 11, 16),
            date(2026, 11, 30),
            members(),
            Vec::new(),
        )
        .unwrap();

        assert_eq!(roster.template_id, template.id);
        assert_eq!(roster.start, date(2026, 11, 16));
        assert_eq!(roster.end, date(2026, 11, 30));
        assert_eq!(roster.status, PlanStatus::Draft);
        assert_eq!(roster.members, members());
        assert_eq!(roster.shifts.len(), 43);
        assert_eq!(roster.total_slots(), 56);
        assert_eq!(roster.filled_slots(), 0);

        let fridays: Vec<NaiveDate> = roster
            .shifts
            .iter()
            .filter(|shift| shift.date == date(2026, 11, 20))
            .map(|shift| shift.date)
            .collect();
        assert!(
            roster
                .shifts
                .iter()
                .all(|shift| shift.name != "OPD" || shift.date != date(2026, 11, 20))
        );
        assert!(
            roster
                .shifts
                .iter()
                .all(|shift| shift.name != "OPD" || shift.date != date(2026, 11, 27))
        );
        assert_eq!(fridays.len(), 2, "Friday still has two emergency shifts");

        for shift in &roster.shifts {
            match shift.name.as_str() {
                "OPD" => assert_eq!(shift.slots.len(), 2),
                "Emergency Morning" | "Emergency Night" => assert_eq!(shift.slots.len(), 1),
                other => panic!("unexpected shift {other}"),
            }
        }

        let night = roster
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
        let mut template = november_template();
        let roster = Roster::create(
            &template,
            "Doctors — snapshot",
            date(2026, 11, 16),
            date(2026, 11, 16),
            members(),
            Vec::new(),
        )
        .unwrap();
        let snapshot = roster
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD shift")
            .clone();

        let opd = template
            .shifts
            .iter_mut()
            .find(|shift| shift.name == "OPD")
            .expect("OPD template");
        opd.name = "Old OPD".to_owned();
        opd.start = at(9, 0);
        opd.weekdays.clear();

        let existing = roster.dated_shift(&snapshot.id).expect("snapshot");
        assert_eq!(existing.name, "OPD");
        assert_eq!(existing.start, at(8, 0));
        assert_eq!(existing.end, at(14, 0));
        assert_eq!(existing.slots, snapshot.slots);

        let regenerated = Roster::create(
            &template,
            "Doctors — regenerated",
            date(2026, 11, 16),
            date(2026, 11, 16),
            members(),
            Vec::new(),
        )
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
        let template = november_template();
        let error = Roster::create(
            &template,
            "Doctors — reversed",
            date(2026, 11, 30),
            date(2026, 11, 16),
            members(),
            Vec::new(),
        )
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
        let template = november_template();
        let context = ContextAssignment {
            date: date(2026, 11, 15),
            name: "Emergency Night".to_owned(),
            start: at(15, 0),
            end: at(8, 0),
            employee_id: "employee-1".to_owned(),
            template_id: None,
        };
        let roster = Roster::create(
            &template,
            "Doctors — context",
            date(2026, 11, 16),
            date(2026, 11, 16),
            members(),
            vec![context.clone()],
        )
        .unwrap();

        assert_eq!(roster.context, vec![context]);
        let dated = &roster.shifts[0];
        let slot = roster
            .slot(&dated.id, &dated.slots[0].id)
            .expect("slot lookup");
        assert_eq!(slot.id, dated.slots[0].id);
        assert!(roster.slot(&dated.id, "missing-slot").is_none());
        assert!(roster.slot("missing-shift", &dated.slots[0].id).is_none());
    }
}
