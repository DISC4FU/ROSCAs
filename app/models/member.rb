class Member < ApplicationRecord
  belongs_to :chama
  has_many :contributions, dependent: :restrict_with_error
  has_many :disbursements, dependent: :restrict_with_error
  has_many :won_payout_cycles,
           class_name: "PayoutCycle",
           foreign_key: :winner_member_id,
           inverse_of: :winner_member,
           dependent: :nullify

  validates :full_name, presence: true
  validates :phone_number,
            presence: true,
            format: { with: /\A254(?:7|1)\d{8}\z/ },
            uniqueness: { scope: :chama_id }
  validates :role, inclusion: { in: %w[member treasurer] }
  validates :status, inclusion: { in: %w[active inactive] }
end
