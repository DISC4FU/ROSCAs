FactoryBot.define do
  factory :audit_log do
    action { "contribution.verified" }
    changeset { { "status" => ["pending", "paid"] } }
    metadata { {} }
  end
end
