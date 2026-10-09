if active
  puts "yes"
elsif pending
  puts "maybe"
else
  puts "no"
end

unless done
  puts "working"
end

if options.key?(:parallel)
  raise ArgumentError, "Sub-groups cannot be parallelized. Use a single group with a very long message here"
end

unless value.is_a?(Hash) && value.keys.all? { |key| key.is_a?(Symbol) }
  raise ArgumentError, "Sub-groups cannot be parallelized."
end

raise ArgumentError, "short" if options.key?(:parallel)

# A condition holding a multi-line operand puts every operand on its own line,
# so nothing of the operand sits in the column of the body below.
def check(node, macos_pair)
  if macos_pair &&
      OnSystemConditionalsHelper::ON_SYSTEM_OPTIONS.map do |m|
        :"on_#{m}"
      end.include?(T.cast(node, RuboCop::AST::BlockNode).method_name) &&
      !allow_macos_depends_in_blocks
    add_offense(node)
  end
end

# An operand that is itself an infix spine wraps at the condition's indent too.
def consistent?(previous, bottle_hash)
  if previous.fetch("formula").slice("path", "pkg_version") ==
      bottle_hash.fetch("formula").slice("path", "pkg_version") &&
      previous.fetch("bottle").slice("root_url", "rebuild", "cellar") ==
          bottle_hash.fetch("bottle").slice("root_url", "rebuild", "cellar")
    true
  end
end
