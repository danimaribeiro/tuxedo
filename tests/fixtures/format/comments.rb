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

# A comment between the arguments of a paren-less call stays above the
# argument it was written against.
fixtures :posts,
  :authors,
  # Reload edges table from fixtures as otherwise repeated test was failing
  :edges

# A comment after the first item of a list is that item's, not the
# opening bracket's.
RELATIONS = [
  :account_invalid, # has_one, without :class_name
  :account_class_name # has_one, with :class_name
]

TEST_TYPES = [
  "Controller tests", # rubocop:disable Style/MutableConstant
  "Helper tests"
]

class A
  X = # :nodoc:
    Foo.merge(a: 1) do |o|
      o
    end
end
