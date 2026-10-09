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

class Directives
  # rubocop:disable Metrics/MethodLength
  def long
    1
  end
  # rubocop:enable Metrics/MethodLength

  def short
    2
  end

  # About `documented`, not `short`.
  def documented
    3
  end
  # rubocop:enable Style/Last
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

def guard_then_redo
  [1, 2].each do |x|
    break if done?

    redo if x == 3
    work(x)
  end
end

# vim: set fileencoding=big5:

puts 1
