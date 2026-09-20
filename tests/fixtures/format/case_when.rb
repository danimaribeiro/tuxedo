case status
when 1
  puts "pending"
when 2, 3
  puts "active or done"
else
  puts "unknown"
end

case
when count > 10
  puts "many"
else
  puts "few"
end

case node.type
when :hash, :array, :str, :sym, :int, :float, :true, :false, :nil, :const # rubocop:disable Lint/BooleanSymbol
  # Primitives and constants are safe
when :send
  1
end
