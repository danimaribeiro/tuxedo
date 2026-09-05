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

Tempfile.open(
  [File.basename(destination), File.extname(destination)],
  File.dirname(destination),
  binmode: true
) do |temp|
  temp.write content
end

PASS_NOT_FOUND = Class.new { def self.action(_); self; end; def self.call(_); [404, {}, []]; end }
