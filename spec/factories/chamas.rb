FactoryBot.define do
  factory :chama do
    name { Faker::Company.unique.name }
    description { Faker::Lorem.sentence }
    contribution_frequency { "monthly" }
    contribution_amount { Faker::Number.decimal(l_digits: 3, r_digits: 2) }
    currency { "KES" }
  end
end
