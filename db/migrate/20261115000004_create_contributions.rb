class CreateContributions < ActiveRecord::Migration[8.1]
  def change
    create_table :contributions do |t|
      t.references :member, null: false, foreign_key: true
      t.references :payout_cycle, null: false, foreign_key: true
      t.decimal :amount, precision: 12, scale: 2, null: false
      t.string :status, null: false, default: "pending"
      t.string :payment_method, null: false, default: "mpesa"
      t.string :checkout_request_id
      t.string :merchant_request_id
      t.string :mpesa_receipt_number
      t.datetime :paid_at

      t.timestamps
    end

    add_index :contributions, [:member_id, :payout_cycle_id], unique: true
    add_index :contributions, :checkout_request_id, unique: true,
                                                   where: "checkout_request_id IS NOT NULL"
    add_check_constraint :contributions, "amount > 0", name: "contributions_amount_positive"
  end
end
