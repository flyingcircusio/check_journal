use chrono::Utc;

pub struct Chrono;

impl Chrono {
    pub fn get_utc_time() -> chrono::DateTime<chrono::Utc> {
        Utc::now()
    }
}
