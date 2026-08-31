# Represents an authenticated user.
class User
  # The default role for new accounts.
  ROLE = :member
  def greet
    name = "Ada" # placeholder until login is wired up
    puts name
  end
end
