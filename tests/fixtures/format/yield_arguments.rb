def each_with_paren
  result = block.arity == 1 ? yield(destination_root) : yield
  result
end

def variants
  yield
  yield 1
  yield a, b
  yield(a, b)
  yield(a)
end
