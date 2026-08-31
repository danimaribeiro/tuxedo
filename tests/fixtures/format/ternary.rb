config = x ? a : b
result = active? ? "yes" : "no"
value = items.empty? ? default : items.first
x = a.is_a?(Hash) ? a.pop : {}
