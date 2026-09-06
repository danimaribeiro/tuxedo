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
