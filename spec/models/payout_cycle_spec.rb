require "rails_helper"

RSpec.describe PayoutCycle, type: :model do
  it { is_expected.to belong_to(:chama) }
  it { is_expected.to have_many(:contributions).dependent(:restrict_with_error) }

  it "requires the end date to follow the start date" do
    cycle = build(:payout_cycle, starts_on: Date.current, ends_on: Date.current)

    expect(cycle).not_to be_valid
    expect(cycle.errors[:ends_on]).to be_present
  end
end
