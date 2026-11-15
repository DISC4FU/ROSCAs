FactoryBot.define do
  factory :contribution do
    payout_cycle
    member { association :member, chama: payout_cycle.chama }
    amount { Faker::Number.decimal(l_digits: 3, r_digits: 2) }
    status { "pending" }
    payment_method { "mpesa" }
    checkout_request_id { "ws_CO_#{SecureRandom.hex(8)}" }
  end
end
