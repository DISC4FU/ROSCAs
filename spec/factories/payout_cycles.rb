FactoryBot.define do
  factory :payout_cycle do
    chama
    sequence(:cycle_number)
    starts_on { Date.current }
    ends_on { Date.current + 1.month }
    payout_on { ends_on }
    status { "scheduled" }
  end
end
