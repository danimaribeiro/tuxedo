foo(
  bar do
    baz
  end,
  other_arg
)

foo(
  ->(x) do
    x + 1
  end,
  other_arg
)

foo(
  begin
    risky
  rescue
    fallback
  end,
  other_arg
)

foo(
  def bar
    1
  end,
  other_arg
)

outer(
  bar(
    if x
      y
    end
  ),
  other_arg
)

foo(
  a: if condition
    "a"
  else
    "b"
  end,
  b: 2
)

{
  a: if condition
    "a"
  else
    "b"
  end,
  b: 2
}

super(
  if condition
    "a"
  else
    "b"
  end,
  other_arg
)

yield(
  if condition
    "a"
  else
    "b"
  end,
  other_arg
)

foo((x if cond), other_arg)
foo(cond ? a : b, other_arg)
