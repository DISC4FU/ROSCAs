require "rails_helper"

RSpec.describe Chama, type: :model do
  it { is_expected.to have_many(:members).dependent(:restrict_with_error) }
  it { is_expected.to have_many(:payout_cycles).dependent(:restrict_with_error) }
  it { is_expected.to validate_presence_of(:name) }
  it { is_expected.to validate_inclusion_of(:contribution_frequency).in_array(%w[weekly biweekly monthly]) }
end
