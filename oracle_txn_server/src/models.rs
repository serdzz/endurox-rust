use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use oracle::Row;
use oracle::sql_type::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub transaction_type: String,
    pub account: String,
    pub amount: i64,
    pub currency: String,
    pub description: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Transaction {
    pub fn from_row(row: &Row) -> Result<Self, oracle::Error> {
        let created_ts: Timestamp = row.get(10)?;
        let updated_ts: Timestamp = row.get(11)?;
        
        // Convert Oracle Timestamp to NaiveDateTime
        let created_at = NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(
                created_ts.year(),
                created_ts.month() as u32,
                created_ts.day() as u32,
            ).unwrap(),
            chrono::NaiveTime::from_hms_opt(
                created_ts.hour() as u32,
                created_ts.minute() as u32,
                created_ts.second() as u32,
            ).unwrap(),
        );
        
        let updated_at = NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(
                updated_ts.year(),
                updated_ts.month() as u32,
                updated_ts.day() as u32,
            ).unwrap(),
            chrono::NaiveTime::from_hms_opt(
                updated_ts.hour() as u32,
                updated_ts.minute() as u32,
                updated_ts.second() as u32,
            ).unwrap(),
        );
        
        Ok(Transaction {
            id: row.get(0)?,
            transaction_type: row.get(1)?,
            account: row.get(2)?,
            amount: row.get::<_, f64>(3)? as i64,
            currency: row.get(4)?,
            description: row.get(5)?,
            status: row.get(6)?,
            message: row.get(7)?,
            error_code: row.get(8)?,
            error_message: row.get(9)?,
            created_at,
            updated_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTransaction {
    pub id: String,
    pub transaction_type: String,
    pub account: String,
    pub amount: i64,
    pub currency: String,
    pub description: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
