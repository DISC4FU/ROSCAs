FactoryBot.define do
  sequence(:kenyan_mobile_number) { |number| "2547#{format('%08d', number)}" }

  factory :member do
    chama
    full_name { Faker::Name.name }
    phone_number { generate(:kenyan_mobile_number) }
    role { "member" }
    status { "active" }
    joined_on { Date.current }
  end
end
