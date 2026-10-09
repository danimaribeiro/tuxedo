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

foo {
  begin
    pool.checkout
    dispose.wait
  rescue => e
    errors << e
  end
}

def empty_rescue
rescue
end

def empty_ensure
ensure
  cleanup
end

begin
rescue
end

# An exception list that does not fit goes one class per line, at one indent,
# the `=> e` staying with the last.
begin
  Formulary.from_rack(rack)
rescue FormulaUnreadableError,
  FormulaClassUnavailableError,
  TapFormulaUnreadableError,
  TapFormulaClassUnavailableError => e
  formula_unavailable_exceptions << e
end
