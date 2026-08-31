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
