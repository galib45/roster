use chrono::NaiveDate;

use crate::{
    models::{ContextAssignment, DoctorId, DutyCategory, RosterProblem},
    utils::{generate_slots, get_doctors_list},
};

mod models;
mod utils;

fn main() {
    let doctors = get_doctors_list();
    let start = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 11, 30).unwrap();
    let slots = generate_slots(start, end);
    let context = vec![ContextAssignment {
        date: NaiveDate::from_ymd_opt(2026, 11, 15).unwrap(),
        category: DutyCategory::EmergencyNight,
        doctor: DoctorId::Amit, // <- put the real Nov 15 night doctor here
    }];
    let roster_problem = RosterProblem {
        doctors,
        slots,
        context,
    };
    let roster = roster_problem.solve().expect("no feasible roster found");
    roster.print();
}
