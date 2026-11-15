class CreateChamas < ActiveRecord::Migration[8.1]
  def change
    create_table :chamas do |t|
      t.string :name, null: false
      t.text :description
      t.string :contribution_frequency, null: false, default: "monthly"
      t.decimal :contribution_amount, precision: 12, scale: 2
      t.string :currency, null: false, default: "KES"

      t.timestamps
    end

    add_check_constraint :chamas,
                         "contribution_amount IS NULL OR contribution_amount > 0",
                         name: "chamas_contribution_amount_positive"
  end
end
