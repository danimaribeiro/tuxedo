items.each { |x| puts x }
3.times { puts "hi" }
context.enter {}
context.enter { context.enter {} }
items.each do |x|
  y = x * 2
  puts y
end
loop do
  puts "spinning"
end

def each
  yield 1
end
