use chrono::{Datelike, NaiveDate, Weekday};

use crate::models::{
    Doctor, DoctorId,
    DutyCategory::{EmergencyMorning, EmergencyNight, Opd},
    DutySlot, Unavailability,
};

pub fn get_doctors_list() -> Vec<Doctor> {
    let mut dr_amit = Doctor::new(DoctorId::Amit);
    dr_amit.add_weekly_unavailability(Weekday::Fri, Unavailability::WholeDay);
    dr_amit.add_weekly_unavailability(Weekday::Sat, Unavailability::WholeDay);
    dr_amit.add_recurring_assignment(Weekday::Sun, EmergencyNight);
    dr_amit.add_recurring_assignment(Weekday::Tue, EmergencyMorning);

    let mut dr_khushi = Doctor::new(DoctorId::Khushi);
    dr_khushi.add_weekly_unavailability(Weekday::Sat, Unavailability::Category(EmergencyNight));
    dr_khushi.add_weekly_unavailability(Weekday::Mon, Unavailability::Category(EmergencyNight));
    dr_khushi.add_weekly_unavailability(Weekday::Wed, Unavailability::Category(EmergencyNight));
    dr_khushi.add_recurring_assignment(Weekday::Sat, EmergencyMorning);
    dr_khushi.add_recurring_assignment(Weekday::Tue, EmergencyNight);

    let mut dr_zakaria = Doctor::new(DoctorId::Zakaria);
    dr_zakaria.add_recurring_assignment(Weekday::Thu, EmergencyNight);
    dr_zakaria.add_recurring_assignment(Weekday::Sun, EmergencyMorning);

    let mut dr_toukir = Doctor::new(DoctorId::Toukir);
    dr_toukir.add_recurring_assignment(Weekday::Sat, EmergencyNight);
    dr_toukir.add_unavailability_on(
        NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        Unavailability::WholeDay,
    );

    let mut dr_sohan = Doctor::new(DoctorId::Sohan);
    dr_sohan.add_recurring_assignment(Weekday::Fri, EmergencyNight);
    dr_sohan.add_recurring_assignment(Weekday::Thu, EmergencyMorning);
    dr_sohan.add_leave_period(
        NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
    );

    let mut dr_atiya = Doctor::new(DoctorId::Atiya);
    dr_atiya.add_recurring_assignment(Weekday::Mon, EmergencyMorning);
    dr_atiya.add_recurring_assignment(Weekday::Wed, EmergencyMorning);
    dr_atiya.add_leave_period(
        NaiveDate::from_ymd_opt(2026, 11, 25).unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 29).unwrap(),
    );

    let mut dr_galib = Doctor::new(DoctorId::Galib);
    dr_galib.add_recurring_assignment(Weekday::Mon, EmergencyNight);
    dr_galib.add_recurring_assignment(Weekday::Wed, EmergencyNight);
    // dr_galib.add_leave_period(
    //     NaiveDate::from_ymd_opt(2026, 11, 25).unwrap(),
    //     NaiveDate::from_ymd_opt(2026, 11, 27).unwrap(),
    // );

    vec![
        dr_amit, dr_khushi, dr_atiya, dr_galib, dr_sohan, dr_toukir, dr_zakaria,
    ]
}

pub fn generate_slots(start: NaiveDate, end: NaiveDate) -> Vec<DutySlot> {
    let mut slots: Vec<DutySlot> = Vec::new();

    let mut date = start;
    while date <= end {
        if date.weekday() != Weekday::Fri {
            slots.push(DutySlot::new(date, Opd));
            slots.push(DutySlot::new(date, Opd));
        }
        slots.push(DutySlot::new(date, EmergencyMorning));
        slots.push(DutySlot::new(date, EmergencyNight));
        date += chrono::Duration::days(1);
    }

    slots
}
