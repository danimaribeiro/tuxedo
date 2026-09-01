colwidth = (array.map { |el| el.to_s.size }.max || 0) + 2
result = ((index + 1) % (Terminal.terminal_width / colwidth)).zero? && !index.zero?
value = (1 + 2) * 3
