pattern = /warning:/
flagged = /foo/i
name = "world"
greeting = "hello #{name}!"
path = "ruby -I #{root}/lib #{root}/spec/fixtures/verbose.thor"
message = /^undefined method `#{name}' for #{Regexp.escape(instance.to_s)}$/
method_name = :"parse_#{option.type}"
