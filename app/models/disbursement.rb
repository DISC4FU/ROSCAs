class Disbursement < ApplicationRecord
  belongs_to :payout_cycle
  belongs_to :member

  validates :amount, numericality: { greater_than: 0 }
  validates :retry_count, numericality: { only_integer: true, greater_than_or_equal_to: 0 }
  validates :status, inclusion: { in: %w[pending processing completed failed] }
  validates :member_id, uniqueness: { scope: :payout_cycle_id }
  validate :member_and_cycle_share_chama

  private

  def member_and_cycle_share_chama
    return if member.nil? || payout_cycle.nil? || member.chama_id == payout_cycle.chama_id

    errors.add(:payout_cycle, "must belong to the member's chama")
  end
end
