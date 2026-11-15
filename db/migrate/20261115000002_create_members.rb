class CreateMembers < ActiveRecord::Migration[8.1]
  def change
    create_table :members do |t|
      t.references :chama, null: false, foreign_key: true
      t.string :full_name, null: false
      t.string :phone_number, null: false
      t.string :role, null: false, default: "member"
      t.string :status, null: false, default: "active"
      t.date :joined_on

      t.timestamps
    end

    add_index :members, [:chama_id, :phone_number], unique: true
  end
end
