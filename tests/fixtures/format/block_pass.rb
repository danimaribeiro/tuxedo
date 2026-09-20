names.each(&:upcase)
total = numbers.reduce(&:+)
register 1, 2, &callback

obj[&blk]
obj[1, &blk]
self[1, &blk]
@obj[1, &blk]
obj()[1, &blk]
klass[1] { |amount| amount * 2 }
klass.foo[1] do |a|
  a
end
klass[1] {}
klass.foo[1] { |a| a }
