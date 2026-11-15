class CreatePayoutCycles < ActiveRecord::Migration[8.1]
  def change
    create_table :payout_cycles do |t|
      t.references :chama, null: false, foreign_key: true
      t.references :winner_member, foreign_key: { to_table: :members }
      t.integer :cycle_number, null: false
      t.date :starts_on, null: false
      t.date :ends_on, null: false
      t.date :payout_on
      t.string :status, null: false, default: "scheduled"

      t.timestamps
    end

    add_index :payout_cycles, [:chama_id, :cycle_number], unique: true
    add_check_constraint :payout_cycles, "cycle_number > 0", name: "payout_cycles_number_positive"
    add_check_constraint :payout_cycles, "ends_on > starts_on", name: "payout_cycles_dates_ordered"
  end
end
