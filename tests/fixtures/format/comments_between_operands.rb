methods = public_instance_methods(true) -
  # Except for public instance methods of Base and its ancestors
  Base.public_instance_methods(true)
ok = timestamp.is_a?(String) &&
  cache_timestamp_format == :usec &&
  # FIXME: checking out a connection for this is wasteful
  connection.supports_precision?
flag = first_check? && # Not using the second one
  second_check? || third_check?
records = source
  .select(&:active?) # only active
  .map(&:name)
  .sort
names = source
  # the names, not the records
  .map(&:name)
  .first
single = foo # why
  .bar(1)
total = (a -
  # why
  b).uniq
