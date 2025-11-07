use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::transactions;

// Diesel Queryable model for reading from database
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name = transactions)]
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

// Diesel Insertable model for creating new records
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = transactions)]
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
