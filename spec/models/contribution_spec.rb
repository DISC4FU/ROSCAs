require "rails_helper"

RSpec.describe Contribution, type: :model do
  it { is_expected.to belong_to(:member) }
  it { is_expected.to belong_to(:payout_cycle) }

  it "rejects a checkout request id already used by another contribution" do
    existing = create(:contribution, checkout_request_id: "ws_CO_test")
    duplicate = build(:contribution,
                      member: create(:member, chama: existing.member.chama),
                      payout_cycle: existing.payout_cycle,
                      checkout_request_id: "ws_CO_test")

    expect(duplicate).not_to be_valid
    expect(duplicate.errors[:checkout_request_id]).to be_present
  end
end
