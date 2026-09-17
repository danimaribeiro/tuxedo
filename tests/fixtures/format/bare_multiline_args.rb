# A sibling of an already-multiline argument moves to its own line.
# The first argument stays on the call line: command style is kept
# (Tuxedo does not add parens).
foo a,
  'hello
world'

foo 'hello
world',
  extra

foo a,
  'hello
world',
  b

arity_asserter.call %w(zero_args one),
  'ERROR: "thor zero_args" was called with arguments ["one"]
Usage: "thor scripts:arities:zero_args"'

obj.call a,
  'hello
world'

super a,
  'hello
world'

def each
  yield a,
    'hello
world'
end

# Nested call that wrapped: the outer command's sibling breaks off.
foo bar(
  argument_number_0,
  argument_number_1,
  argument_number_2,
  argument_number_3,
  argument_number_4,
  argument_number_5,
  argument_number_6
),
  extra

# A single multiline argument is already one-per-line; leave it.
foo 'hello
world'

# Short commands stay flat.
foo a, b, c
# A single overlong argument still cannot wrap without adding parens.
foo "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
