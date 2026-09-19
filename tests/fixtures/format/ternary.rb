config = x ? a : b
result = active? ? "yes" : "no"
value = items.empty? ? default : items.first
x = a.is_a?(Hash) ? a.pop : {}

def guarded
  valid? ? proceed : raise("invalid")
  cleanup
end

result = some_condition_that_is_quite_long? ?
  some_long_method_call_for_the_truthy_case(argument) :
  another_long_fallback_method_call(argument)

def replace_keys(record, force: false)
  target_key_values = record ?
    ActiveRecord::Key.for(primary_key(record.class)).map { |col| record.read_attribute(col) } :
    []
  owner_key_values = foreign_key.map { |fk| owner.read_attribute(fk) }
end

# A ternary in an argument list nests one level deeper than the call.
def nested
  foo(
    cond ?
      some_long_method_call_for_the_truthy_case(argument_number_one, argument_two) :
      another_long_fallback_method_call(argument)
  )
end
