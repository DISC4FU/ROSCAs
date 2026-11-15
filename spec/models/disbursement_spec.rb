require "rails_helper"

RSpec.describe Disbursement, type: :model do
  it { is_expected.to belong_to(:member) }
  it { is_expected.to belong_to(:payout_cycle) }

  it "does not allow a negative retry count" do
    expect(build(:disbursement, retry_count: -1)).not_to be_valid
  end
end
