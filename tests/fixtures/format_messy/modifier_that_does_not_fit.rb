raise ArgumentError, "Sub-groups cannot be parallelized. Use a single group with a very long message here" if options.key?(:parallel)
raise ArgumentError, "Sub-groups cannot be parallelized." unless value.is_a?(Hash) && value.keys.all? { |key| key.is_a?(Symbol) }
x = compute_something_really_long(argument_one, argument_two) if some_really_long_condition_name.present?
raise ArgumentError, "short" if options.key?(:parallel)
foo { raise ArgumentError, "Sub-groups cannot be parallelized. Use a single group with a very long message" if options.key?(:parallel) }
begin
  step
end while queue.any_pending_work_that_is_long_enough_to_push_this_line_past_the_limit_of_one_hundred_and_twenty
raise ArgumentError, "Sub-groups cannot be parallelized. Use a single group with a very long message here" if options.key?(:parallel) # about the statement
return if control.empty? && cache_control.empty? # stays a modifier, comment kept
