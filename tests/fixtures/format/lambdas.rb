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

foo = ->(
  name,
  description,
  long_description,
  wrap_long_description_and_extra_field_more,
  usage,
  options_value_extra_long
) do
  name
end
