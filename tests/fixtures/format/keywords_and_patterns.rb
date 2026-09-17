y = 1
puts "a" if defined?(Foo)
puts 1 if defined? @a
case v
in Integer | Float => n
  n
in [*, 3, *post]
  1
in Foo(*, { a: }, *)
  1
in ^y
  2
in ^(y + 1)
  3
end
END {
  bye
  bye2
}
BEGIN {}
undef foo, :bar
puts(1) if (a == 1)..(b == 2)
# shareable_constant_value: literal

X = [1, 2]
at_exit { END { z } }
