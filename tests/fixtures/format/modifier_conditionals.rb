puts "yes" if active
yield unless pretend?
return if done?

raise "error" unless valid?

class Foo
end if Process.respond_to?(:fork)

def foo
  1
end if bar?

def method_option(name, options = {})
  unless [Symbol, String].any? { |klass| name.is_a?(klass) }
    raise ArgumentError, "Expected a Symbol or String, got #{name.inspect}"
  end

  scope = 1
end
