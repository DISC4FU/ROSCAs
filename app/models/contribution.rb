class Contribution < ApplicationRecord
  belongs_to :member
  belongs_to :payout_cycle

  validates :amount, numericality: { greater_than: 0 }
  validates :status, inclusion: { in: %w[pending paid failed reversed] }
  validates :payment_method, presence: true
  validates :checkout_request_id, uniqueness: true, allow_nil: true
  validates :member_id, uniqueness: { scope: :payout_cycle_id }
  validate :member_and_cycle_share_chama

  private

  def member_and_cycle_share_chama
    return if member.nil? || payout_cycle.nil? || member.chama_id == payout_cycle.chama_id

    errors.add(:payout_cycle, "must belong to the member's chama")
  end
end
