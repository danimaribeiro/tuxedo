# A condition that breaks sits one level deeper than the body, so the two
# are not drawn identically.

if first_condition_is_long_enough_here && second_condition_is_also_quite_long && third_one_as_well_here && fourth_condition_here
  body_statement_one
end

unless (value.is_a?(Integer) && value >= 0) || value == Float::INFINITY || value == -Float::INFINITY || value.is_a?(Symbol) || value.is_a?(Proc)
  raise ArgumentError, ":key must be a non-negative Integer, Infinity, Symbol, or Proc"
end

while current.is_a?(Prism::ParenthesesNode) && current.body.is_a?(Prism::StatementsNode) && current.body.body.length == 1
  current = current.body.body.first
end

# An argument list carries its own indentation, so a condition built out
# of one is left where it was.
if @has_messages.try_acquire(argument_number_one, argument_number_two, argument_number_three, argument_number_four)
  message = @messages.pop(true)
end

# No line point in the condition: nothing to move.
if short && condition
  body
end
