app = lambda { |env| req = Request.new(env); res = make_response!(req); dispatch(req, res) }
items.each { |x| y = x + 1; puts y }
