# Represents an authenticated user.
class User
  # The default role for new accounts.
  ROLE = :member
  def greet
    name = "Ada" # placeholder until login is wired up
    puts name
  end

  def deprecated # :nodoc:
    warn "old"
  end

  def dispatch(command)
    if command == :old
      legacy
    elsif recognized = lookup(command) # rubocop:disable Lint/AssignmentInCondition
      recognized
    else
      unknown
    end
  end
end
