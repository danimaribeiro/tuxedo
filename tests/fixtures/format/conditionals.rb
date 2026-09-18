if active
  puts "yes"
elsif pending
  puts "maybe"
else
  puts "no"
end

unless done
  puts "working"
end

if options.key?(:parallel)
  raise ArgumentError, "Sub-groups cannot be parallelized. Use a single group with a very long message here"
end

unless value.is_a?(Hash) && value.keys.all? { |key| key.is_a?(Symbol) }
  raise ArgumentError, "Sub-groups cannot be parallelized."
end

raise ArgumentError, "short" if options.key?(:parallel)
