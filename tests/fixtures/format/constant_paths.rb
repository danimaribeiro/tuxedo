x = Thor::DynamicCommand
y = A::B::C
z = ::Top
w = ::Top::Nested
instance = Foo::Bar.new

def dynamic_command_class
  Thor::DynamicCommand
end

def top_level
  ::Top::Nested
end
