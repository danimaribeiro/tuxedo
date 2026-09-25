STRICT_DEFAULTS = {
  allow_external_references: false,
  allow_sha1: false,

  require_signing_time: true,
  require_signing_certificate: true,

  # Grouped by the author, with a comment.
  max_clock_skew: 300,
  # A comment with a gap under it.

  trusted_roots: []
}

PERMISSIONS = [
  "manage_organization_billing",
  "manage_organization_members",

  "publish_articles_to_the_front_page",
  "moderate_comments_on_any_article"
]

configure(
  adapter: :postgresql,
  database: "application_production",

  pool: ENV.fetch("DATABASE_POOL_SIZE", 5),
  timeout: ENV.fetch("DATABASE_TIMEOUT", 5000)
)

validates :organization_identifier,
  :organization_display_name,
  presence: true,

  uniqueness: { scope: :account_identifier, case_sensitive: false, message: "is already taken in this account" }
