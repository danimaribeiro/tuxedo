method = $1.strip
matched = $&
last_match = $~
size = `stty size`.split
listing = %x(ls -la)
interpolated = `echo #{name}`
