x = "foo" "bar"
y = "a#{b}c" "d"
message = "one " "two " "three"
single = 'raw ' 'parts'
escaped = "quote \" here " "and more"
puts "warning: " "details follow"
error = "Failed to execute 'close' on WebSocket: " \
  "The code must be either 1000, or between 3000 and 4999. " \
  "#{code} is neither."

# An overflowing concat nested inside an argument list or an array wraps at
# its own part boundaries too, one indent in from the item it belongs to --
# the group is composed into the enclosing one rather than rendered flat.
raise RecordNotSaved.new(
  "Failed to remove the existing associated record. " \
    "The record failed to save after its foreign key was set to nil.",
  target
)
warn(
  [
    "500 Internal Server Error\n" \
      "If you are the administrator of this website, then please read this web " \
      "application's log file."
  ]
)
