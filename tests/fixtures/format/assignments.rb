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
a, b, = compute
a, *, b = compute
@a, @b = 1, 2
@@x, @@y = 1, 2
$p, $q = 1, 2
First, Second = 1, 2
Config::Left, Config::Right = 1, 2
obj.left, obj.right = 1, 2
h[:a], h[:b] = 1, 2
h["k"], other = 1, 2
first, (second, third) = 1, [2, 3]
first, (second, (third, fourth)) = 1, [2, [3, 4]]

def tally
  total = 0
  total += 1
  puts total
end

# A multiple assignment whose right side ends in a collection expands only
# that last value, which stays on the line the assignment began on.
prev_configs, ActiveRecord::Base.configurations = ActiveRecord::Base.configurations, {
  "arunit" => { "primary" => { "adapter" => "sqlite3" } }
}

def long_values
  expected =
    "ALTER TABLE `people` ADD `last_name` varchar(255), ADD `first_name` varchar(255), ALGORITHM = INPLACE, LOCK = NONE"
  @message ||=
    "ALTER TABLE `people` ADD `last_name` varchar(255), ADD `first_name` varchar(255), ALGORITHM = INPLACE, LOCK = NONE"
  headers["WWW-Authenticate"] =
    %(Digest realm="#{realm}", qop="auth", algorithm=MD5, nonce="#{nonce}", opaque="#{opaque}", more="more")
  ActionController::Parameters.always_permitted_parameters_with_a_long_name =
    app.config.action_controller.always_permitted_parameters_with_a_long_name
  short = "stays on the line"
  too_long_to_help = "a string so long that moving it under the target would not bring it within the limit of a line, nothing at all, however long this string is made to be"
end
