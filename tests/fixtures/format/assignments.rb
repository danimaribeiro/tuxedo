x = 1
x += 2
x -= 1
x ||= 3
x &&= 4
@count = 0
@count += 1
@count ||= 0
@count &&= 1
Total = 0
Total += 1
Total ||= 0
Total &&= 1
$global = 0
$global += 1
$global ||= 0
$global &&= 1
@@shared = 0
@@shared += 1
@@shared ||= 0
@@shared &&= 1
Config::Limit = 10
Config::Limit += 1
a, b = 1, 2
a, *b = [1, 2, 3]
*a, b = [1, 2, 3]
a, b = compute

def tally
  total = 0
  total += 1
  puts total
end
