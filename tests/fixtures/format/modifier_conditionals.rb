puts "yes" if active
yield unless pretend?
return if done?
raise "error" unless valid?

class Foo
end if Process.respond_to?(:fork)

def foo
  1
end if bar?
