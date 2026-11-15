use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{postgres::Postgres, Transaction};
const MAX_CONTRIBUTION_AMOUNT: i64 = 9_999_999_999;

#[derive(Deserialize)]
pub(crate) struct StkPushInput {
    member_id: i64,
    payout_cycle_id: i64,
    amount: i64,
}

#[derive(Serialize)]
pub(crate) struct StkPushAccepted {
    contribution_id: i64,
    merchant_request_id: String,
    checkout_request_id: String,
}

#[derive(Serialize)]
pub(crate) struct CallbackAcknowledgement {
    received: bool,
    duplicate: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum CallbackDisposition {
    Processed,
    Duplicate,
}

#[derive(Deserialize)]
struct CallbackEnvelope {
    #[serde(rename = "Body")]
    body: CallbackBody,
}

#[derive(Deserialize)]
struct CallbackBody {
    #[serde(rename = "stkCallback")]
    stk_callback: StkCallback,
}

#[derive(Deserialize)]
struct StkCallback {
    #[serde(rename = "MerchantRequestID")]
    merchant_request_id: String,
    #[serde(rename = "CheckoutRequestID")]
    checkout_request_id: String,
    #[serde(rename = "ResultCode")]
    result_code: i64,
    #[serde(rename = "ResultDesc")]
    result_desc: String,
    #[serde(rename = "CallbackMetadata")]
    callback_metadata: Option<CallbackMetadata>,
}

#[derive(Deserialize)]
struct CallbackMetadata {
    #[serde(rename = "Item")]
    items: Vec<CallbackItem>,
}

#[derive(Deserialize)]
struct CallbackItem {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Value")]
    value: Option<Value>,
}

#[derive(Debug)]
pub(crate) enum ApiError {
    BadRequest(&'static str),
    NotFound,
    Conflict,
    Unprocessable(&'static str),
    ServiceUnavailable,
    BadGateway,
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound => (StatusCode::NOT_FOUND, "record not found"),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "a contribution already exists for this cycle",
            ),
            Self::Unprocessable(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            Self::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Daraja integration is not configured",
            ),
            Self::BadGateway => (
                StatusCode::BAD_GATEWAY,
                "Daraja did not accept the STK Push request",
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "payment processing failed",
            ),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub(crate) async fn initiate_stk_push(
    State(state): State<AppState>,
    Json(input): Json<StkPushInput>,
) -> Result<(StatusCode, Json<StkPushAccepted>), ApiError> {
    if input.amount <= 0 || input.amount > MAX_CONTRIBUTION_AMOUNT {
        return Err(ApiError::BadRequest(
            "amount must be a positive whole number within the contribution limit",
        ));
    }

    let daraja = state.daraja.as_ref().ok_or(ApiError::ServiceUnavailable)?;
    let member = sqlx::query!(
        r#"
        SELECT
            members.phone_number AS member_phone_number,
            members.chama_id AS member_chama_id,
            payout_cycles.chama_id AS cycle_chama_id
        FROM members
        JOIN payout_cycles ON payout_cycles.id = $2
        WHERE members.id = $1
        "#,
        input.member_id,
        input.payout_cycle_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(database_error)?
    .ok_or(ApiError::NotFound)?;

    if member.member_chama_id != member.cycle_chama_id {
        return Err(ApiError::Unprocessable(
            "member and payout cycle must belong to the same chama",
        ));
    }
    if !is_valid_kenyan_mobile_number(&member.member_phone_number) {
        return Err(ApiError::Unprocessable(
            "member does not have a valid Kenyan mobile number",
        ));
    }

    let contribution_id = sqlx::query_scalar!(
        r#"
        INSERT INTO contributions (
            member_id, payout_cycle_id, amount, status, payment_method, created_at, updated_at
        )
        VALUES ($1, $2, ($3::bigint)::numeric, 'pending', 'mpesa', NOW(), NOW())
        RETURNING id
        "#,
        input.member_id,
        input.payout_cycle_id,
        input.amount
    )
    .fetch_one(&state.db)
    .await
    .map_err(database_error)?;

    let stk_result = match daraja
        .initiate_stk_push(&member.member_phone_number, input.amount)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(%error, contribution_id, "Daraja STK Push initiation failed");
            let mut transaction = state.db.begin().await.map_err(database_error)?;
            insert_audit(
                &mut transaction,
                contribution_id,
                "mpesa_stk_initiation_error",
                json!({}),
                json!({ "error": error.to_string() }),
            )
            .await?;
            transaction.commit().await.map_err(database_error)?;
            return Err(ApiError::BadGateway);
        }
    };

    let mut transaction = state.db.begin().await.map_err(database_error)?;
    let updated = sqlx::query!(
        r#"
        UPDATE contributions
        SET merchant_request_id = $1, checkout_request_id = $2, updated_at = NOW()
        WHERE id = $3 AND status = 'pending'
        "#,
        stk_result.merchant_request_id,
        stk_result.checkout_request_id,
        contribution_id
    )
    .execute(&mut *transaction)
    .await
    .map_err(database_error)?;
    if updated.rows_affected() != 1 {
        return Err(ApiError::Conflict);
    }

    insert_audit(
        &mut transaction,
        contribution_id,
        "mpesa_stk_initiated",
        json!({ "status": { "from": null, "to": "pending" } }),
        json!({
            "merchant_request_id": stk_result.merchant_request_id,
            "checkout_request_id": stk_result.checkout_request_id
        }),
    )
    .await?;
    transaction.commit().await.map_err(database_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(StkPushAccepted {
            contribution_id,
            merchant_request_id: stk_result.merchant_request_id,
            checkout_request_id: stk_result.checkout_request_id,
        }),
    ))
}

pub(crate) async fn mpesa_callback(
    State(state): State<AppState>,
    Json(raw_payload): Json<Value>,
) -> Result<Json<CallbackAcknowledgement>, ApiError> {
    let envelope: CallbackEnvelope =
        serde_json::from_value(raw_payload.clone()).map_err(|error| {
            tracing::warn!(%error, "invalid Daraja callback payload");
            ApiError::BadRequest("invalid Daraja callback payload")
        })?;

    let mut transaction = state.db.begin().await.map_err(database_error)?;
    let disposition =
        process_callback(&mut transaction, &envelope.body.stk_callback, &raw_payload).await?;
    transaction.commit().await.map_err(database_error)?;

    Ok(Json(CallbackAcknowledgement {
        received: true,
        duplicate: disposition == CallbackDisposition::Duplicate,
    }))
}

async fn process_callback(
    transaction: &mut Transaction<'_, Postgres>,
    callback: &StkCallback,
    raw_payload: &Value,
) -> Result<CallbackDisposition, ApiError> {
    let contribution = sqlx::query!(
        r#"
        SELECT
            contributions.id,
            contributions.status,
            contributions.merchant_request_id,
            contributions.amount::text AS "amount!",
            members.phone_number
        FROM contributions
        JOIN members ON members.id = contributions.member_id
        WHERE contributions.checkout_request_id = $1
        FOR UPDATE OF contributions
        "#,
        callback.checkout_request_id
    )
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?
    .ok_or(ApiError::NotFound)?;

    if contribution.merchant_request_id.as_deref() != Some(&callback.merchant_request_id) {
        return Err(ApiError::Unprocessable(
            "callback merchant request ID does not match the contribution",
        ));
    }

    if contribution.status != "pending" {
        tracing::info!(
            checkout_request_id = %callback.checkout_request_id,
            "ignoring duplicate Daraja callback"
        );
        return Ok(CallbackDisposition::Duplicate);
    }

    if callback.result_code == 0 {
        process_success_callback(
            transaction,
            callback,
            raw_payload,
            contribution.id,
            &contribution.amount,
            &contribution.phone_number,
        )
        .await?;
    } else {
        sqlx::query!(
            r#"
            UPDATE contributions
            SET status = 'failed', raw_callback_payload = $1, updated_at = NOW()
            WHERE id = $2 AND status = 'pending'
            "#,
            raw_payload.clone(),
            contribution.id
        )
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;

        insert_audit(
            transaction,
            contribution.id,
            "mpesa_callback_failed",
            json!({ "status": { "from": "pending", "to": "failed" } }),
            json!({
                "checkout_request_id": callback.checkout_request_id,
                "merchant_request_id": callback.merchant_request_id,
                "result_code": callback.result_code,
                "result_description": callback.result_desc
            }),
        )
        .await?;
    }

    Ok(CallbackDisposition::Processed)
}

async fn process_success_callback(
    transaction: &mut Transaction<'_, Postgres>,
    callback: &StkCallback,
    raw_payload: &Value,
    contribution_id: i64,
    expected_amount: &str,
    expected_phone_number: &str,
) -> Result<(), ApiError> {
    let metadata = callback
        .callback_metadata
        .as_ref()
        .ok_or(ApiError::Unprocessable(
            "successful callback is missing transaction metadata",
        ))?;

    let amount = metadata
        .value("Amount")
        .and_then(parse_whole_amount)
        .ok_or(ApiError::Unprocessable(
            "successful callback has an invalid amount",
        ))?;
    let expected_amount =
        parse_whole_amount(&Value::String(expected_amount.to_owned())).ok_or(ApiError::Internal)?;
    if amount != expected_amount {
        return Err(ApiError::Unprocessable(
            "callback amount does not match the contribution",
        ));
    }

    let phone_number = metadata
        .value("PhoneNumber")
        .and_then(value_as_string)
        .ok_or(ApiError::Unprocessable(
            "successful callback is missing the payer phone number",
        ))?;
    if phone_number != expected_phone_number {
        return Err(ApiError::Unprocessable(
            "callback phone number does not match the member",
        ));
    }

    let receipt_number = metadata
        .value("MpesaReceiptNumber")
        .and_then(value_as_string)
        .filter(|value| !value.is_empty())
        .ok_or(ApiError::Unprocessable(
            "successful callback is missing the M-Pesa receipt number",
        ))?;

    sqlx::query!(
        r#"
        UPDATE contributions
        SET status = 'paid',
            mpesa_receipt_number = $1,
            raw_callback_payload = $2,
            paid_at = NOW(),
            updated_at = NOW()
        WHERE id = $3 AND status = 'pending'
        "#,
        receipt_number,
        raw_payload.clone(),
        contribution_id
    )
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    insert_audit(
        transaction,
        contribution_id,
        "mpesa_callback_succeeded",
        json!({
            "status": { "from": "pending", "to": "paid" },
            "mpesa_receipt_number": receipt_number
        }),
        json!({
            "checkout_request_id": callback.checkout_request_id,
            "merchant_request_id": callback.merchant_request_id,
            "result_code": callback.result_code,
            "result_description": callback.result_desc,
            "transaction_date": metadata.value("TransactionDate")
                .and_then(value_as_string)
        }),
    )
    .await?;

    Ok(())
}

async fn insert_audit(
    transaction: &mut Transaction<'_, Postgres>,
    contribution_id: i64,
    action: &str,
    changeset: Value,
    metadata: Value,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"
        INSERT INTO audit_logs (
            actor_type, actor_id, auditable_type, auditable_id,
            action, changeset, metadata, created_at
        )
        VALUES ('Daraja', NULL, 'Contribution', $1, $2, $3, $4, NOW())
        "#,
        contribution_id,
        action,
        changeset,
        metadata
    )
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

impl CallbackMetadata {
    fn value(&self, name: &str) -> Option<&Value> {
        self.items
            .iter()
            .find(|item| item.name == name)
            .and_then(|item| item.value.as_ref())
    }
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn parse_whole_amount(value: &Value) -> Option<i64> {
    let value = value_as_string(value)?;
    let (whole, fractional) = value.split_once('.').unwrap_or((&value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional.bytes().all(|byte| byte == b'0')
    {
        return None;
    }
    whole.parse().ok()
}

fn is_valid_kenyan_mobile_number(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 12
        && &bytes[..3] == b"254"
        && matches!(bytes[3], b'1' | b'7')
        && bytes[4..].iter().all(u8::is_ascii_digit)
}

fn database_error(error: sqlx::Error) -> ApiError {
    if error
        .as_database_error()
        .and_then(|database_error| database_error.code())
        .as_deref()
        == Some("23505")
    {
        return ApiError::Conflict;
    }
    tracing::error!(%error, "payment database operation failed");
    ApiError::Internal
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::{env, time::SystemTime};

    #[test]
    fn parses_amounts_from_numbers_and_strings() {
        assert_eq!(parse_whole_amount(&json!(100)), Some(100));
        assert_eq!(parse_whole_amount(&json!(100.0)), Some(100));
        assert_eq!(parse_whole_amount(&json!("100")), Some(100));
        assert_eq!(parse_whole_amount(&json!("100.00")), Some(100));
        assert_eq!(parse_whole_amount(&json!("100.25")), None);
        assert_eq!(parse_whole_amount(&json!(-100)), None);
    }

    #[test]
    fn accepts_only_valid_kenyan_mobile_numbers() {
        assert!(is_valid_kenyan_mobile_number("254712345678"));
        assert!(is_valid_kenyan_mobile_number("254112345678"));
        assert!(!is_valid_kenyan_mobile_number("0712345678"));
        assert!(!is_valid_kenyan_mobile_number("254812345678"));
    }

    #[tokio::test]
    async fn duplicate_success_callback_updates_once_and_audits_once() {
        let database_url =
            env::var("DATABASE_URL").expect("DATABASE_URL must point to a migrated test database");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .unwrap();
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let mut transaction = pool.begin().await.unwrap();

        let chama = sqlx::query!(
            r#"
            INSERT INTO chamas (name, contribution_frequency, currency, created_at, updated_at)
            VALUES ($1, 'monthly', 'KES', NOW(), NOW())
            RETURNING id
            "#,
            format!("callback-test-{suffix}")
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        let member = sqlx::query!(
            r#"
            INSERT INTO members (
                chama_id, full_name, phone_number, role, status, created_at, updated_at
            )
            VALUES ($1, 'Callback Test Member', '254712345678', 'member', 'active', NOW(), NOW())
            RETURNING id
            "#,
            chama.id
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        let cycle = sqlx::query!(
            r#"
            INSERT INTO payout_cycles (
                chama_id, cycle_number, starts_on, ends_on, status, created_at, updated_at
            )
            VALUES ($1, 1, CURRENT_DATE, CURRENT_DATE + 1, 'collecting', NOW(), NOW())
            RETURNING id
            "#,
            chama.id
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        let checkout_request_id = format!("ws_CO_duplicate_{suffix}");
        let contribution = sqlx::query!(
            r#"
            INSERT INTO contributions (
                member_id, payout_cycle_id, amount, status, payment_method,
                merchant_request_id, checkout_request_id, created_at, updated_at
            )
            VALUES ($1, $2, ($3::bigint)::numeric, 'pending', 'mpesa', 'merchant-test', $4, NOW(), NOW())
            RETURNING id
            "#,
            member.id,
            cycle.id,
            100_i64,
            checkout_request_id
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        let payload = json!({
            "Body": {
                "stkCallback": {
                    "MerchantRequestID": "merchant-test",
                    "CheckoutRequestID": checkout_request_id,
                    "ResultCode": 0,
                    "ResultDesc": "The service request is processed successfully.",
                    "CallbackMetadata": {
                        "Item": [
                            { "Name": "Amount", "Value": 100.0 },
                            { "Name": "MpesaReceiptNumber", "Value": "TESTRCPT01" },
                            { "Name": "TransactionDate", "Value": 20261115101500_i64 },
                            { "Name": "PhoneNumber", "Value": "254712345678" }
                        ]
                    }
                }
            }
        });
        let envelope: CallbackEnvelope = serde_json::from_value(payload.clone()).unwrap();

        let first = process_callback(&mut transaction, &envelope.body.stk_callback, &payload)
            .await
            .unwrap();
        let duplicate = process_callback(&mut transaction, &envelope.body.stk_callback, &payload)
            .await
            .unwrap();

        assert_eq!(first, CallbackDisposition::Processed);
        assert_eq!(duplicate, CallbackDisposition::Duplicate);
        let stored = sqlx::query!(
            r#"
            SELECT status, mpesa_receipt_number, raw_callback_payload IS NOT NULL AS "has_payload!"
            FROM contributions
            WHERE id = $1
            "#,
            contribution.id
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        assert_eq!(stored.status, "paid");
        assert_eq!(stored.mpesa_receipt_number.as_deref(), Some("TESTRCPT01"));
        assert!(stored.has_payload);

        let audit_count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!"
            FROM audit_logs
            WHERE auditable_type = 'Contribution'
              AND auditable_id = $1
              AND action = 'mpesa_callback_succeeded'
            "#,
            contribution.id
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        assert_eq!(audit_count, 1);
        transaction.rollback().await.unwrap();
    }
}
