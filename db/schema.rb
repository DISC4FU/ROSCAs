# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[8.1].define(version: 2026_11_15_000007) do
  # These are extensions that must be enabled in order to support this database
  enable_extension "pg_catalog.plpgsql"

  create_table "audit_logs", force: :cascade do |t|
    t.string "actor_type"
    t.bigint "actor_id"
    t.string "auditable_type"
    t.bigint "auditable_id"
    t.string "action", null: false
    t.jsonb "changeset", default: {}, null: false
    t.jsonb "metadata", default: {}, null: false
    t.datetime "created_at", default: -> { "CURRENT_TIMESTAMP" }, null: false
    t.index ["actor_type", "actor_id"], name: "index_audit_logs_on_actor"
    t.index ["auditable_type", "auditable_id"], name: "index_audit_logs_on_auditable"
  end

  create_table "chamas", force: :cascade do |t|
    t.string "name", null: false
    t.text "description"
    t.string "contribution_frequency", default: "monthly", null: false
    t.decimal "contribution_amount", precision: 12, scale: 2
    t.string "currency", default: "KES", null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.check_constraint "contribution_amount IS NULL OR contribution_amount > 0::numeric", name: "chamas_contribution_amount_positive"
  end

  create_table "contributions", force: :cascade do |t|
    t.bigint "member_id", null: false
    t.bigint "payout_cycle_id", null: false
    t.decimal "amount", precision: 12, scale: 2, null: false
    t.string "status", default: "pending", null: false
    t.string "payment_method", default: "mpesa", null: false
    t.string "checkout_request_id"
    t.string "merchant_request_id"
    t.string "mpesa_receipt_number"
    t.datetime "paid_at"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.jsonb "raw_callback_payload"
    t.index ["checkout_request_id"], name: "index_contributions_on_checkout_request_id", unique: true, where: "(checkout_request_id IS NOT NULL)"
    t.index ["member_id", "payout_cycle_id"], name: "index_contributions_on_member_id_and_payout_cycle_id", unique: true
    t.index ["member_id"], name: "index_contributions_on_member_id"
    t.index ["payout_cycle_id"], name: "index_contributions_on_payout_cycle_id"
    t.check_constraint "amount > 0::numeric", name: "contributions_amount_positive"
  end

  create_table "disbursements", force: :cascade do |t|
    t.bigint "payout_cycle_id", null: false
    t.bigint "member_id", null: false
    t.decimal "amount", precision: 12, scale: 2, null: false
    t.string "status", default: "pending", null: false
    t.integer "retry_count", default: 0, null: false
    t.string "conversation_id"
    t.string "originator_conversation_id"
    t.string "mpesa_receipt_number"
    t.text "result_description"
    t.datetime "completed_at"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["member_id"], name: "index_disbursements_on_member_id"
    t.index ["payout_cycle_id", "member_id"], name: "index_disbursements_on_payout_cycle_id_and_member_id", unique: true
    t.index ["payout_cycle_id"], name: "index_disbursements_on_payout_cycle_id"
    t.check_constraint "amount > 0::numeric", name: "disbursements_amount_positive"
    t.check_constraint "retry_count >= 0", name: "disbursements_retry_count_nonnegative"
  end

  create_table "members", force: :cascade do |t|
    t.bigint "chama_id", null: false
    t.string "full_name", null: false
    t.string "phone_number", null: false
    t.string "role", default: "member", null: false
    t.string "status", default: "active", null: false
    t.date "joined_on"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["chama_id", "phone_number"], name: "index_members_on_chama_id_and_phone_number", unique: true
    t.index ["chama_id"], name: "index_members_on_chama_id"
  end

  create_table "payout_cycles", force: :cascade do |t|
    t.bigint "chama_id", null: false
    t.bigint "winner_member_id"
    t.integer "cycle_number", null: false
    t.date "starts_on", null: false
    t.date "ends_on", null: false
    t.date "payout_on"
    t.string "status", default: "scheduled", null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["chama_id", "cycle_number"], name: "index_payout_cycles_on_chama_id_and_cycle_number", unique: true
    t.index ["chama_id"], name: "index_payout_cycles_on_chama_id"
    t.index ["winner_member_id"], name: "index_payout_cycles_on_winner_member_id"
    t.check_constraint "cycle_number > 0", name: "payout_cycles_number_positive"
    t.check_constraint "ends_on > starts_on", name: "payout_cycles_dates_ordered"
  end

  add_foreign_key "contributions", "members"
  add_foreign_key "contributions", "payout_cycles"
  add_foreign_key "disbursements", "members"
  add_foreign_key "disbursements", "payout_cycles"
  add_foreign_key "members", "chamas"
  add_foreign_key "payout_cycles", "chamas"
  add_foreign_key "payout_cycles", "members", column: "winner_member_id"
end
