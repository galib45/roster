use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::models::DutyCategory::{EmergencyMorning, EmergencyNight, Opd};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DutyCategory {
    Opd,
    EmergencyMorning,
    EmergencyNight,
}

impl DutyCategory {
    pub fn hours(self) -> u16 {
        match self {
            Opd => 6,
            EmergencyMorning => 7,
            EmergencyNight => 17,
        }
    }
}

#[derive(Debug)]
pub enum Unavailability {
    WholeDay,
    Category(DutyCategory),
}

#[derive(Debug)]
pub struct WeeklyUnavailability {
    pub weekday: Weekday,
    pub scope: Unavailability,
}

#[derive(Debug)]
pub struct DateUnavailability {
    pub date: NaiveDate,
    pub scope: Unavailability,
}

#[derive(Debug)]
pub struct DateRange {
    start: NaiveDate,
    end: NaiveDate,
}

impl DateRange {
    pub fn contains(&self, date: NaiveDate) -> bool {
        self.start <= date && date <= self.end
    }
}

#[derive(Debug)]
pub struct RecurringAssignment {
    pub weekday: Weekday,
    pub category: DutyCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DoctorId {
    Amit,
    Khushi,
    Zakaria,
    Toukir,
    Sohan,
    Atiya,
    Galib,
}

impl DoctorId {
    pub fn name(&self) -> &'static str {
        match self {
            DoctorId::Amit => "Dr. Amit",
            DoctorId::Khushi => "Dr. Khushi",
            DoctorId::Zakaria => "Dr. Zakaria",
            DoctorId::Toukir => "Dr. Toukir",
            DoctorId::Sohan => "Dr. Sohan",
            DoctorId::Atiya => "Dr. Atiya",
            DoctorId::Galib => "Dr. Galib",
        }
    }
}

#[derive(Debug)]
pub struct ContextAssignment {
    pub date: NaiveDate,
    pub category: DutyCategory,
    pub doctor: DoctorId,
}

#[derive(Debug)]
pub struct Doctor {
    pub id: DoctorId,
    pub weekly_unavailabilities: Vec<WeeklyUnavailability>,
    pub date_unavailabilities: Vec<DateUnavailability>,
    pub leave_periods: Vec<DateRange>,
    pub recurring_assignments: Vec<RecurringAssignment>,
}

impl Doctor {
    pub fn new(id: DoctorId) -> Self {
        Self {
            id,
            weekly_unavailabilities: Vec::new(),
            date_unavailabilities: Vec::new(),
            leave_periods: Vec::new(),
            recurring_assignments: Vec::new(),
        }
    }

    pub fn add_weekly_unavailability(&mut self, weekday: Weekday, scope: Unavailability) {
        self.weekly_unavailabilities
            .push(WeeklyUnavailability { weekday, scope });
    }

    pub fn add_unavailability_on(&mut self, date: NaiveDate, scope: Unavailability) {
        self.date_unavailabilities
            .push(DateUnavailability { date, scope });
    }

    pub fn add_recurring_assignment(&mut self, weekday: Weekday, category: DutyCategory) {
        self.recurring_assignments
            .push(RecurringAssignment { weekday, category });
    }

    pub fn add_leave_period(&mut self, start: NaiveDate, end: NaiveDate) {
        self.leave_periods.push(DateRange { start, end });
    }

    pub fn is_unavailable(&self, slot: DutySlot) -> bool {
        self.leave_periods.iter().any(|r| r.contains(slot.date))
            || self.date_unavailabilities.iter().any(|u| {
                u.date == slot.date
                    && match u.scope {
                        Unavailability::WholeDay => true,
                        Unavailability::Category(c) => c == slot.category,
                    }
            })
            || self.weekly_unavailabilities.iter().any(|u| {
                u.weekday == slot.date.weekday()
                    && match u.scope {
                        Unavailability::WholeDay => true,
                        Unavailability::Category(c) => c == slot.category,
                    }
            })
    }

    pub fn is_recurring(&self, slot: DutySlot) -> bool {
        self.recurring_assignments
            .iter()
            .any(|r| r.weekday == slot.date.weekday() && r.category == slot.category)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DutySlot {
    pub date: NaiveDate,
    pub category: DutyCategory,
}

impl DutySlot {
    pub fn new(date: NaiveDate, category: DutyCategory) -> Self {
        Self { date, category }
    }
}

pub struct Roster<'a> {
    problem: &'a RosterProblem,
    /// One doctor index per slot, aligned with problem.slots
    doctor_of_slot: Vec<usize>,
}

impl<'a> Roster<'a> {
    pub fn slot_at(&self, slot_idx: usize) -> &DutySlot {
        &self.problem.slots[slot_idx]
    }

    pub fn doctor_for_slot(&self, slot_idx: usize) -> &'a Doctor {
        let problem: &'a RosterProblem = self.problem;
        &problem.doctors[self.doctor_of_slot[slot_idx]]
    }

    pub fn print(&self) {
        // Slots are generated in date order, so the dates are already sorted;
        // collecting unique ones is just "has the date changed?".
        let mut dates: Vec<NaiveDate> = Vec::new();
        for s in 0..self.problem.slots.len() {
            let date = self.slot_at(s).date;
            if dates.last() != Some(&date) {
                dates.push(date);
            }
        }

        println!(
            "{:<12} {:<5} {:<22} {:<22} {:<22}",
            "Date", "Day", "EmergencyMorning", "EmergencyNight", "Opd"
        );
        println!("{}", "-".repeat(86));

        for &date in &dates {
            // Gather this day's assignments from the slot array
            let mut em = None;
            let mut en = None;
            let mut opd: Vec<&str> = Vec::new();

            for (s, _) in self.doctor_of_slot.iter().enumerate() {
                let slot = self.slot_at(s);
                if slot.date != date {
                    continue;
                }
                let name = self.doctor_for_slot(s).id.name();
                match slot.category {
                    EmergencyMorning => em = Some(name),
                    EmergencyNight => en = Some(name),
                    Opd => opd.push(name),
                }
            }

            println!(
                "{:<12} {:<5} {:<22} {:<22} {:<22}",
                date,
                format!("{:?}", date.weekday()),
                em.unwrap_or("-"),
                en.unwrap_or("-"),
                opd.join(", "),
            );
        }

        // Per-doctor totals (unchanged from before)
        println!("\n{:<12} {}", "Doctor", "Hours");
        println!("{}", "-".repeat(24));
        for (d, doc) in self.problem.doctors.iter().enumerate() {
            let hours: u16 = self
                .doctor_of_slot
                .iter()
                .enumerate()
                .filter(|&(_, &dd)| dd == d)
                .map(|(s, _)| self.slot_at(s).category.hours())
                .sum();
            println!("{:<12} {}h", doc.id.name(), hours);
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Neighbor {
    Slot(usize),        // slot index -> check assigned[slot]
    FixedDoctor(usize), // doctor index -> that doctor is already there
}

#[derive(Debug)]
struct Prepared {
    allowed: Vec<Vec<bool>>,
    same_day: Vec<Vec<usize>>,
    next_em: Vec<Option<Neighbor>>,
    prev_en: Vec<Option<Neighbor>>,
    // context: HashMap<(NaiveDate, DutyCategory), usize>,
}

#[derive(Debug)]
pub struct RosterProblem {
    pub doctors: Vec<Doctor>,
    pub slots: Vec<DutySlot>,
    pub context: Vec<ContextAssignment>,
}

impl RosterProblem {
    fn prepare(&self) -> Result<Prepared, String> {
        let n_slots = self.slots.len();
        let mut slots_of: HashMap<NaiveDate, Vec<usize>> = HashMap::new();
        for (s, slot) in self.slots.iter().enumerate() {
            slots_of.entry(slot.date).or_default().push(s);
        }

        // Index every slot by date and category
        // let mut opd_of: HashMap<NaiveDate, Vec<usize>> = HashMap::new();
        let mut em_of: HashMap<NaiveDate, usize> = HashMap::new();
        let mut en_of: HashMap<NaiveDate, usize> = HashMap::new();
        for (s, slot) in self.slots.iter().enumerate() {
            match slot.category {
                // Opd => opd_of.entry(slot.date).or_default().push(s),
                Opd => {}
                EmergencyMorning => {
                    em_of.insert(slot.date, s);
                }
                EmergencyNight => {
                    en_of.insert(slot.date, s);
                }
            }
        }

        let mut context: HashMap<(NaiveDate, DutyCategory), usize> = HashMap::new();
        for c in &self.context {
            let d = self
                .doctors
                .iter()
                .position(|doc| doc.id == c.doctor)
                .ok_or_else(|| format!("context refers to unknown doctor {:?}", c.doctor))?;
            context.insert((c.date, c.category), d);
        }

        let mut same_day: Vec<Vec<usize>> = vec![Vec::new(); n_slots];
        for idxs in slots_of.values() {
            for i in 0..idxs.len() {
                for j in (i + 1)..idxs.len() {
                    same_day[idxs[i]].push(idxs[j]);
                    same_day[idxs[j]].push(idxs[i]);
                }
            }
        }

        // Night slot -> next day's morning slot (rest rule, forward direction)
        // Night slot -> next day's morning slot (rest rule, forward direction)
        let next_em = self
            .slots
            .iter()
            .map(|slot| {
                if slot.category != EmergencyNight {
                    return None;
                }
                let next_date = slot.date + chrono::Duration::days(1);
                if let Some(&n) = em_of.get(&next_date) {
                    return Some(Neighbor::Slot(n));
                }
                context
                    .get(&(next_date, EmergencyMorning))
                    .copied()
                    .map(Neighbor::FixedDoctor)
            })
            .collect();

        // Morning slot -> previous day's night slot (rest rule, backward direction)
        // Morning slot -> previous day's night slot (rest rule, backward direction)
        let prev_en = self
            .slots
            .iter()
            .map(|slot| {
                if slot.category == EmergencyNight {
                    return None;
                }
                let prev_date = slot.date - chrono::Duration::days(1);
                if let Some(&n) = en_of.get(&prev_date) {
                    return Some(Neighbor::Slot(n));
                }
                context
                    .get(&(prev_date, EmergencyNight))
                    .copied()
                    .map(Neighbor::FixedDoctor)
            })
            .collect();

        let allowed = self
            .slots
            .iter()
            .map(|slot| {
                self.doctors
                    .iter()
                    .map(|doc| !doc.is_unavailable(*slot))
                    .collect()
            })
            .collect();

        Ok(Prepared {
            allowed,
            same_day,
            next_em,
            prev_en,
            // context,
        })
    }

    /// Doctors who can legally take slot `s`, best candidate first.
    fn viable_doctors(
        &self,
        s: usize,
        assigned: &[Option<usize>],
        load: &[u32],
        prepared: &Prepared,
    ) -> Vec<usize> {
        let slot = self.slots[s];

        let mut candidates: Vec<usize> = (0..self.doctors.len())
            .filter(|&d| prepared.allowed[s][d])
            .filter(|&d| {
                // Same-day rule: not the sibling OPD slot of this date
                prepared.same_day[s].iter().all(|&c| assigned[c] != Some(d))
            })
            .filter(|&d| {
                // Rest rule, checked from both directions so it holds
                // no matter which of the two slots gets assigned first

                match slot.category {
                    EmergencyNight => prepared.next_em[s].map_or(true, |n| match n {
                        Neighbor::Slot(n) => assigned[n] != Some(d),
                        Neighbor::FixedDoctor(doc) => doc != d,
                    }),
                    EmergencyMorning | Opd => prepared.prev_en[s].map_or(true, |n| match n {
                        Neighbor::Slot(n) => assigned[n] != Some(d),
                        Neighbor::FixedDoctor(doc) => doc != d,
                    }),
                }
            })
            .collect();

        // Least-loaded first; recurring owner first on ties
        candidates.sort_by_key(|&d| {
            let recurring = u32::from(self.doctors[d].is_recurring(slot));
            (load[d], 1 - recurring)
        });
        candidates
    }

    fn most_constrained_slot(
        &self,
        assigned: &[Option<usize>],
        load: &[u32],
        prepared: &Prepared,
    ) -> Option<(usize, Vec<usize>)> {
        let mut best: Option<(usize, Vec<usize>)> = None;
        for s in 0..self.slots.len() {
            if assigned[s].is_some() {
                continue;
            }
            let candidates = self.viable_doctors(s, assigned, load, prepared);
            if candidates.is_empty() {
                return Some((s, candidates)); // dead end: fail fast on this slot
            }
            if best
                .as_ref()
                .map_or(true, |(_, best_c)| candidates.len() < best_c.len())
            {
                best = Some((s, candidates));
            }
        }
        best
    }

    fn search(
        &self,
        assigned: &mut Vec<Option<usize>>,
        load: &mut Vec<u32>,
        prepared: &Prepared,
        budget: &mut u32,
    ) -> Result<(), String> {
        // Safety valve: never hang forever on pathological data
        *budget = budget.saturating_sub(1);
        if *budget == 0 {
            return Err("search budget exhausted (1M steps)".into());
        }

        // Nothing left to decide? The roster is complete.
        let Some((s, candidates)) = self.most_constrained_slot(assigned, load, prepared) else {
            return Ok(());
        };
        if candidates.is_empty() {
            return Err("no viable doctor for this slot".into());
        }

        let slot = self.slots[s];
        let hours = slot.category.hours() as u32;

        // Try each candidate in fairness order: assign, recurse, undo.
        for d in candidates {
            assigned[s] = Some(d);
            load[d] += hours;

            if self.search(assigned, load, prepared, budget).is_ok() {
                return Ok(()); // a full solution exists below us; keep it
            }

            // Undo — exactly reversing the two lines above
            assigned[s] = None;
            load[d] -= hours;
        }

        Err(format!(
            "no doctor works for {:?} {:?} (all candidates failed)",
            slot.date, slot.category
        ))
    }

    pub fn solve(&self) -> Result<Roster<'_>, String> {
        let n_slots = self.slots.len();
        let prepared = self.prepare()?;

        let mut assigned: Vec<Option<usize>> = vec![None; n_slots];
        for (s, slot) in self.slots.iter().enumerate() {
            for (d, doc) in self.doctors.iter().enumerate() {
                if !doc.is_recurring(*slot) {
                    continue;
                }
                if !prepared.allowed[s][d] {
                    eprintln!(
                        "note: {} is recurring for {:?} on {} but unavailable; skipping",
                        doc.id.name(),
                        slot.category,
                        slot.date
                    );
                } else if let Some(other) = assigned[s] {
                    return Err(format!(
                        "slot {:?} {:?}: recurring conflict between {} and {}",
                        slot.date,
                        slot.category,
                        self.doctors[other].id.name(),
                        doc.id.name()
                    ));
                } else {
                    assigned[s] = Some(d);
                }
            }
        }

        let mut load: Vec<u32> = vec![0; self.doctors.len()];
        for (s, d) in assigned.iter().enumerate() {
            if let Some(d) = *d {
                load[d] += self.slots[s].category.hours() as u32;
            }
        }
        // if let Some(s) = assigned.iter().position(|a| a.is_none()) {
        //     let v = self.viable_doctors(s, &assigned, &load, &prepared);
        //     let names: Vec<&str> = v.iter().map(|&d| self.doctors[d].id.name()).collect();
        //     eprintln!(
        //         "first undecided slot {}: {:?} {:?} -> viable: {:?}",
        //         s, self.slots[s].date, self.slots[s].category, names
        //     );
        // }
        // Err("search not implemented yet".into())
        let mut budget: u32 = 1_000_000;
        self.search(&mut assigned, &mut load, &prepared, &mut budget)?;

        Ok(Roster {
            problem: self,
            doctor_of_slot: assigned
                .iter()
                .map(|a| a.expect("search succeeded but left a slot empty"))
                .collect(),
        })
    }
}
