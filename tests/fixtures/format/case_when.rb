case status
when 1
  puts "pending"
when 2, 3
  puts "active or done"
else
  puts "unknown"
end

case
when count > 10
  puts "many"
else
  puts "few"
end

case node.type
when :hash, :array, :str, :sym, :int, :float, :true, :false, :nil, :const # rubocop:disable Lint/BooleanSymbol
  # Primitives and constants are safe
when :send
  1
end

def coerce_signature_policy(value)
  case value
  when SignaturePolicy then value
  when Hash then SignaturePolicy.new(**value)
  when :implied, "implied" then SignaturePolicy.implied # both spellings
  when nil
    nil
  when String then SignaturePolicy.parse(value)
    # A comment below a one-line branch stays with it.
  else
    raise ArgumentError, "cannot coerce #{value.class}"
  end
end

case key
when :organization_identifier
  "written out by the author, so it stays written out"
when :organization_display_name
  lookup_table_for_organizations.fetch(value).display_name_for_locale(current_locale, fallback: true)
end

case shape
in { type: :circle, radius: } then Math::PI * radius**2
in [width, height] then width * height
end

# A comment among the values of a `when` puts them one per line, each comment
# where it was written.
case path
when "cps", # Common Package Specification database
  "pkgconfig", # pkg-config database
  /^gio/
  :mkpath
when
  # Check for `http://` GitHub homepage URLs.
  %r{^http://github\.com/},
  # Savannah has full SSL/TLS support.
  %r{^http://savannah\.nongnu\.org/}
  :secure
end
