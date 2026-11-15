FactoryBot.define do
  factory :disbursement do
    payout_cycle
    member { association :member, chama: payout_cycle.chama }
    amount { Faker::Number.decimal(l_digits: 4, r_digits: 2) }
    status { "pending" }
    retry_count { 0 }
  end
end
