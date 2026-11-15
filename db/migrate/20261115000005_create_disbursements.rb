class CreateDisbursements < ActiveRecord::Migration[8.1]
  def change
    create_table :disbursements do |t|
      t.references :payout_cycle, null: false, foreign_key: true
      t.references :member, null: false, foreign_key: true
      t.decimal :amount, precision: 12, scale: 2, null: false
      t.string :status, null: false, default: "pending"
      t.integer :retry_count, null: false, default: 0
      t.string :conversation_id
      t.string :originator_conversation_id
      t.string :mpesa_receipt_number
      t.text :result_description
      t.datetime :completed_at

      t.timestamps
    end

    add_index :disbursements, [:payout_cycle_id, :member_id], unique: true
    add_check_constraint :disbursements, "amount > 0", name: "disbursements_amount_positive"
    add_check_constraint :disbursements, "retry_count >= 0",
                         name: "disbursements_retry_count_nonnegative"
  end
end
