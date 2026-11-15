class Chama < ApplicationRecord
  has_many :members, dependent: :restrict_with_error
  has_many :payout_cycles, dependent: :restrict_with_error
  has_many :contributions, through: :members
  has_many :disbursements, through: :payout_cycles

  validates :name, presence: true
  validates :contribution_frequency, inclusion: { in: %w[weekly biweekly monthly] }
  validates :contribution_amount, numericality: { greater_than: 0 }, allow_nil: true
  validates :currency, presence: true, length: { is: 3 }
end
