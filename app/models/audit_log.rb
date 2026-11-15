class AuditLog < ApplicationRecord
  belongs_to :actor, polymorphic: true, optional: true
  belongs_to :auditable, polymorphic: true, optional: true

  validates :action, presence: true
end
