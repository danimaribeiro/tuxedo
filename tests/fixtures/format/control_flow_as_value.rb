foo(
  if condition
    "a"
  else
    "b"
  end,
  other_arg
)

x = [
  if condition
    "a"
  else
    "b"
  end,
  other_arg
]

foo(
  case value
  when 1
    "one"
  else
    "other"
  end,
  other_arg
)

longer_variable_name = if condition
  "a"
else
  "b"
end

type = case value
when Symbol
  :sym
when Numeric
  :num
end

result = unless flag
  1
else
  2
end

# An operator value that wraps indents one fixed level, rather than
# landing under the `=`. Both of these break to the same column even
# though their targets differ in length: the old code divided the
# target's width by the indent size and floored it, so renaming the
# variable moved the value's indentation.
result_value_with_a_long_name = some_predicate_with_a_really_quite_long_name_here &&
  another_predicate_with_a_much_longer_name_here_indeed_truly_yes_ok

r = some_predicate_with_a_really_quite_long_name_here &&
  another_predicate_with_a_much_longer_name_here_indeed_truly_yes_ok
