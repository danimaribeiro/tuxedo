# frozen_string_literal: true

class Table
  def rows
    a = 1
    # tuxedo:off
    matrix = [
      [1,   0,   0],
      [0,   1,   0],
      [0,   0,   1],
    ]
    text   =   <<~EOS
      keep   this
    EOS
    # tuxedo:on
    b = 2
    [a, b]
  end

  def tail
    x = 1
    # tuxedo:off
    y   =   2
    # tuxedo:on
  end

  def after
    z = 3
  end
end

# tuxedo:off
w   =   5
v   =   6
# tuxedo:on
u = 7
