status = :ok
label = :"two words"
price = 19.99
range = 1..5
exclusive = 1...5
config = { host: "localhost", port: 80 }
mapped = { "k" => "v" }
empty = {}
quoted = "she said \"hi\""
multiline_escape = "line one\nline two"
help_mappings = ["-h", "-?", "--help", "-D"]
rational = 2.123456789r
imaginary = 5i

# A range that is a statement rather than part of an assignment: it still
# sits at the statement's indent, on a line of its own.
def all_day
  beginning_of_day..end_of_day
end
