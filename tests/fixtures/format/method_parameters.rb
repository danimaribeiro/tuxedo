def foo(a, b = 1, *rest, c, x:, y: 2, **kw, &block)
  a
end

def bar(*args)
  args
end

def baz(**opts)
  opts
end

def qux(&block)
  block
end

def anon(*, **, &)
  1
end

def overlong_parameter_list(
  name,
  description,
  long_description,
  wrap_long_description_and_extra,
  usage,
  options = nil,
  options_relation = nil
)
  1
end
