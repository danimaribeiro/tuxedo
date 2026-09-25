loader.do_not_eager_load(
  "#{lib}/action_cable/subscription_adapter/redis.rb",
  "#{lib}/action_cable/subscription_adapter/postgresql.rb",

  "#{lib}/action_cable/test_helper.rb"
)
template = handler.new(
  details.handler_class,

  format: details.format_or_default,
  variant: variant&.to_s,

  locals: locals.map(&:to_s)
)
X = { a: 1,

  b: 2 }
foo(a,

  b)
