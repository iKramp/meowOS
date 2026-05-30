use super::Timer;
use crate::utils::{byte_from_port, byte_to_port};

const CMOS_REG_SELECT: u16 = 0x70;
const CMOS_REG_DATA: u16 = 0x71;

pub(super) struct RtcWrapper;
pub(super) const RTC_WRAPPER: RtcWrapper = RtcWrapper;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RtcValues {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day_of_week: u8,
    day_of_month: u8,
    month: u8,
    year: u16,
}

impl RtcValues {
    fn new() -> Self {
        RtcValues {
            seconds: 0,
            minutes: 0,
            hours: 0,
            day_of_week: 0,
            day_of_month: 0,
            month: 0,
            year: 0,
        }
    }

    fn read() -> Self {
        byte_to_port(CMOS_REG_SELECT, 0x0A);
        while byte_from_port(CMOS_REG_DATA) & 0x80 != 0 {}

        //seconds
        byte_to_port(CMOS_REG_SELECT, 0x00);
        let seconds = bcd_to_bin(byte_from_port(CMOS_REG_DATA));

        //minutes
        byte_to_port(CMOS_REG_SELECT, 0x02);
        let minutes = bcd_to_bin(byte_from_port(CMOS_REG_DATA));

        //hours
        byte_to_port(CMOS_REG_SELECT, 0x04);
        let mut hours = bcd_to_bin(byte_from_port(CMOS_REG_DATA));
        if hours & 0x80 != 0 {
            //12-hour format, convert to 24-hour format
            hours = (hours & 0x7F) + 12;
        }

        //day of week
        byte_to_port(CMOS_REG_SELECT, 0x06);
        let day_of_week = bcd_to_bin(byte_from_port(CMOS_REG_DATA));

        //day of month
        byte_to_port(CMOS_REG_SELECT, 0x07);
        let day_of_month = bcd_to_bin(byte_from_port(CMOS_REG_DATA));

        //month
        byte_to_port(CMOS_REG_SELECT, 0x08);
        let month = bcd_to_bin(byte_from_port(CMOS_REG_DATA));

        //year
        byte_to_port(CMOS_REG_SELECT, 0x09);
        let year = bcd_to_bin(byte_from_port(CMOS_REG_DATA)) as u16 + 2000;

        RtcValues {
            seconds,
            minutes,
            hours,
            day_of_week,
            day_of_month,
            month,
            year,
        }
    }
}

fn bcd_to_bin(bcd: u8) -> u8 {
    ((bcd >> 4) * 10) + (bcd & 0x0F)
}

impl core::convert::From<RtcValues> for std::time::Instant {
    fn from(rtc: RtcValues) -> Self {
        let seconds_since_epoch = rtc.seconds as u64
            + rtc.minutes as u64 * 60
            + rtc.hours as u64 * 3600
            + seconds_on_date_since_new_year(rtc.year, rtc.month, rtc.day_of_month)
            + secons_on_year_since_epoch(rtc.year);

        let duration = std::time::Duration::from_secs(seconds_since_epoch);
        std::time::Instant::from_duration_since_epoch(duration)
    }
}

fn seconds_on_date_since_new_year(year: u16, month: u8, day: u8) -> u64 {
    let mut days = 0;
    for m in 1..month {
        days += match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) {
                    29
                } else {
                    28
                }
            }
            _ => unreachable!(),
        };
    }
    days += day as u64 - 1; // subtract one because we count from the start of the month
    days * 24 * 3600 // convert days to seconds
}

fn secons_on_year_since_epoch(year: u16) -> u64 {
    let mut seconds = 0;
    for y in 1970..year {
        seconds += if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
            366 * 24 * 3600 // leap year
        } else {
            365 * 24 * 3600 // non-leap year
        };
    }
    seconds
}

impl Timer for RtcWrapper {
    fn init(&mut self) -> bool {
        true //nothing to do here
    }

    fn get_time(&self) -> std::time::Instant {
        let mut old_values = RtcValues {
            seconds: 0,
            minutes: 0,
            hours: 0,
            day_of_week: 0,
            day_of_month: 0,
            month: 0,
            year: 0,
        };

        loop {
            let new_values = RtcValues::read();
            if new_values == old_values {
                break;
            }
            old_values = new_values;
        }

        old_values.into()
    }

    fn calibrate(&mut self, _now: std::time::Instant) {
        //RTC is not a high-resolution timer, nothing to calibrate
    }
}
