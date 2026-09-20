items.each { |x| puts x }
3.times { puts "hi" }
context.enter {}
context.enter { context.enter {} }
items.each do |x|
  y = x * 2
  puts y
end
loop do
  puts "spinning"
end

def each
  yield 1
end

Tempfile.open(
  [File.basename(destination), File.extname(destination)],
  File.dirname(destination),
  binmode: true
) do |temp|
  temp.write content
end

PASS_NOT_FOUND = Class.new {
  def self.action(_)
    self
  end

  def self.call(_)
    [404, {}, []]
  end
}

x = Class.new {
  if true
    1
  elsif false
    2
  else
    3
  end
}
x = Class.new {
  unless true
    1
  else
    2
  end
}
x = Class.new {
  while true
    break
  end
}
x = Class.new {
  until true
    break
  end
}
x = Class.new {
  case 1
  when 1
    :a
  when 2
    :b
  else
    :c
  end
}
x = Class.new {
  case y
  in 1
    :a
  in 2
    :b
  else
    :c
  end
}

@mutex.synchronize {
  @executor ||= ThreadedExecutor.new(max_size: config.executor_pool_size, name: "streamer", wait: true)
}

callback = ->(request, extra_long_parameter_name) {
  tag.respond_to?(:call) ? tag.call(request ||= block.call) : tag.to_s.camelize
}

def app
  @app || @app_build_lock.synchronize {
    @app ||= begin
      stack = default_middleware_stack
      config.middleware.build(endpoint)
    end
  }
end

# A `{ }` body with more than one statement keeps them on their own lines
# wherever it appears -- including inside an argument list, where it used
# to be captured as text and collapsed onto one line with `;`.
queue_adapter.with(
  stopping: ->(job) {
    jobs << job
    false
  }
) do
  perform_enqueued_jobs
end

router.get { |*_| }
pool.each { |worker| }

queries.map! { |query|
  if query.one?
    query.first
  else
    query
  end
}
Class.new {
  def create_table(name)
  end
}
foo {
  x = case y
  when 1
    :one
  end
}
foo {
  while queue.any?
    queue.shift
  end
}
foo {
  begin
    risky
  rescue StandardError
    retry_later
  end
}
foo { value ? 1 : 2 }
foo { value if ready? }
included.class_methods {
  def foo
    @foo << :included
  end
} # rubocop:disable Lint/NestedMethodDefinition

foo { |k,| bar }
foo { |a, b,| bar }
proc { |a, (b, c),| 1 }
