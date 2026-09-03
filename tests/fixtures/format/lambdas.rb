identity = ->(x) { x }
add = ->(a, b) { a + b }
constant = -> { 42 }
noop = ->(_) { nil }
handlers = { on_save: ->(v) { v * 2 } }

multi = ->(x) do
  x + 1
end

def wrap(block)
  app = ->(_) { block.call }
  app
end
