x = "foo" \
  "bar"
y = "a#{b}c" \
  "d"
message = "one " \
  "two " \
  "three"
single = "raw " \
  "parts"
escaped = "quote \" here " \
  "and more"
puts "warning: " \
  "details follow"
error = "Failed to execute 'close' on WebSocket: " \
  "The code must be either 1000, or between 3000 and 4999. " \
  "#{code} is neither."

# An overflowing concat nested inside an argument list or an array wraps at
# its own part boundaries too, one indent in from the item it belongs to --
# the group is composed into the enclosing one rather than rendered flat.
raise RecordNotSaved.new(
  "Failed to remove the existing associated record. " \
    "The record failed to save after its foreign key was set to nil in the database.",
  target
)
warn(
  [
    "500 Internal Server Error\n" \
      "If you are the administrator of this website, then please read this web " \
      "application's log file."
  ]
)

name = "#{config.table_name_prefix}" \
  "#{fixture_set_name.tr('/', '_')}"

# A concatenation is one deliberate construct, so it breaks even when it
# would fit, and its parts are all-or-nothing in an argument list too.
raise ArgumentError,
  "Cannot specify :compressor option when using" \
    " default serializer and cache format version is < 7.1"

# What follows an operator at the end of a continued string continues from
# that line's indent.
message = "Mismatch between cputypes >> 0x%08<fat_cputype>x and 0x%08<macho_cputype>x\n" \
  "and/or cpusubtypes >> 0x%08<fat_cpusubtype>x and 0x%08<macho_cpusubtype>x" % {
    fat_cputype: fat_cputype,
    macho_cputype: macho_cputype,
    fat_cpusubtype: fat_cpusubtype,
    macho_cpusubtype: macho_cpusubtype
  }
