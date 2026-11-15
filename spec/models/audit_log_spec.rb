require "rails_helper"

RSpec.describe AuditLog, type: :model do
  it { is_expected.to belong_to(:actor).optional }
  it { is_expected.to belong_to(:auditable).optional }
  it { is_expected.to validate_presence_of(:action) }
end
