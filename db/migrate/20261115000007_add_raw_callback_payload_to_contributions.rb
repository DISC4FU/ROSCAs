class AddRawCallbackPayloadToContributions < ActiveRecord::Migration[8.1]
  def change
    add_column :contributions, :raw_callback_payload, :jsonb
  end
end
