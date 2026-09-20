pattern = /warning:/
flagged = /foo/i
name = "world"
greeting = "hello #{name}!"
path = "ruby -I #{root}/lib #{root}/spec/fixtures/verbose.thor"
message = /^undefined method `#{name}' for #{Regexp.escape(instance.to_s)}$/
method_name = :"parse_#{option.type}"

# A `#{}` hole stays on one line however long the result. Breaking it is valid
# Ruby -- the hole is code -- but the continuation would sit at a column that
# belongs to the string around it.
report = "Could not handle incoming message: #{message.inspect} [#{e.class} - #{e.message}]: #{e.backtrace.first(5).join(" | ")}"
summary = "#{queries.size} instead of #{count} queries were executed#{queries.empty? ? '' : ". Queries:\n\n#{queries.join("\n\n")}"}"

if /#{f(1, 2)}/
  y
end
