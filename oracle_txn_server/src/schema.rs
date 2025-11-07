// Diesel schema for transactions table
// @generated automatically by Diesel CLI.

diesel::table! {
    transactions (id) {
        id -> Varchar,
        transaction_type -> Varchar,
        account -> Varchar,
        amount -> BigInt,
        currency -> Varchar,
        description -> Nullable<Varchar>,
        status -> Varchar,
        message -> Nullable<Varchar>,
        error_code -> Nullable<Varchar>,
        error_message -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
