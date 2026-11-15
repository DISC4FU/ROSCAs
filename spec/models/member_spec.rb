require "rails_helper"

RSpec.describe Member, type: :model do
  it { is_expected.to belong_to(:chama) }
  it { is_expected.to validate_presence_of(:full_name) }

  it "requires a valid Kenyan mobile number" do
    member = build(:member, phone_number: "0700123456")

    expect(member).not_to be_valid
    expect(member.errors[:phone_number]).to be_present
  end

  it "requires phone numbers to be unique within a chama" do
    existing_member = create(:member, phone_number: "254712345678")
    duplicate = build(:member, chama: existing_member.chama, phone_number: existing_member.phone_number)

    expect(duplicate).not_to be_valid
    expect(duplicate.errors[:phone_number]).to be_present
  end
end
