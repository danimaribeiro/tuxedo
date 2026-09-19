def m
  m.call(request) && (constraints.empty? || constraints.all? { |method, value|
    case value
    when Regexp
      a
    else
      b
    end
  })
end

x = constraints.all? { |method, value|
  case value
  when Regexp
    a
  else
    b
  end
}
