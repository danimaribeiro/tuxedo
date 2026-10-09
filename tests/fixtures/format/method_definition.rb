def greet(name)
  puts name
end

def foo = 42

def self.x = 1

def f(...) = g(...)

def a(x) # note
  x + 1
end

def long_method_name(argument_one, argument_two)
  some_receiver.call_something(argument_one, argument_two, argument_three, 1)
end

# A comment at the end of a one-line endless definition stays with it.
def present? = !empty? # :nodoc:
