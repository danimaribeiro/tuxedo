class Foo
  def a
    1
  end

  def b
    2
  end

  private

  def c
    3
  end

  attr_reader :x
  attr_reader :y

  def d
    4
  end
end

def guard_example
  return unless valid?

  do_stuff
end

def guard_run
  return 1 unless x
  return 2 unless y
  if z
    raise "z"
  end

  do_stuff
end

def guard_run_spaced
  return 1 unless x

  return 2 unless y

  do_stuff
end

# vim: set fileencoding=big5:

puts 1
