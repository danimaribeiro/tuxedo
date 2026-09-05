begin
  risky
rescue Foo
  handle_foo
rescue Bar => e
  handle_bar(e)
else
  ok
ensure
  cleanup
end

def foo
  bar
rescue StandardError => e
  handle(e)
ensure
  cleanup
end

command = begin
  foo
end

command = Class.new(Thor) do
  foo
end

transmit rescue nil
socket.close

foo { begin; pool.checkout; dispose.wait; rescue => e; errors << e; end }
