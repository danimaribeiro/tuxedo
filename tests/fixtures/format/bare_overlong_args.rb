# Overlong bare commands wrap one argument per line. First arg stays
# on the call line; parens are not added.
assert_equal "long payload",
  long_queue.pop(timeout: WAIT_WHEN_EXPECTING_EVENT),
  "broadcast to a >63-byte multibyte channel must reach its own subscribers"

env = Rack::MockRequest.env_for "/test",
  "HTTP_CONNECTION" => "upgrade",
  "HTTP_UPGRADE" => "websocket",
  "HTTP_HOST" => "localhost",
  "HTTP_ORIGIN" => "http://rubyonrails.com"

raise Foo,
  "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

super a,
  argument_number_0,
  argument_number_1,
  argument_number_2,
  argument_number_3,
  argument_number_4,
  argument_number_5,
  argument_number_6

def each
  yield a,
    argument_number_0,
    argument_number_1,
    argument_number_2,
    argument_number_3,
    argument_number_4,
    argument_number_5,
    argument_number_6
end

# Short commands stay flat.
foo a, b, c

# A single overlong argument cannot wrap without adding parens.
foo "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
