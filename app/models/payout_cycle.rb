class PayoutCycle < ApplicationRecord
  belongs_to :chama
  belongs_to :winner_member, class_name: "Member", optional: true, inverse_of: :won_payout_cycles
  has_many :contributions, dependent: :restrict_with_error
  has_many :disbursements, dependent: :restrict_with_error

  validates :cycle_number, numericality: { only_integer: true, greater_than: 0 },
                           uniqueness: { scope: :chama_id }
  validates :starts_on, :ends_on, presence: true
  validates :status, inclusion: { in: %w[scheduled collecting ready paid cancelled] }
  validate :ends_after_start
  validate :winner_belongs_to_chama

  private

  def ends_after_start
    return if starts_on.blank? || ends_on.blank? || ends_on > starts_on

    errors.add(:ends_on, "must be after the start date")
  end

  def winner_belongs_to_chama
    return if winner_member.nil? || winner_member.chama_id == chama_id

    errors.add(:winner_member, "must belong to the same chama")
  end
end
