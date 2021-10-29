use chrono::Utc;

pub struct Chrono;

impl Chrono {
    pub fn get_utc_time() -> chrono::DateTime<chrono::Utc> {
        Utc::now()
    }

    pub fn get_utc_timestamp_as_rfc2822() -> String {
        crate::Chrono::get_utc_time().to_rfc2822()
    }
}
