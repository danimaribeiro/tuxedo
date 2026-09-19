case action_name
when "alpha_one", "bravo_two", "charlie_three", "delta_four", "echo_five", "foxtrot_six", "golf_seven", "hotel_eight", "india_nine"
  render template: "test/hello_world"
when "a", "b"
  1
end
