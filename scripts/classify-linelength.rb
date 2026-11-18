#!/usr/bin/env ruby
# frozen_string_literal: true

# Splits over-long lines into the ones a formatter could still fix and the
# ones it could not.
#
# Usage:
#   scripts/classify-linelength.rb <dir> [--limit N] [--list N] [--sites FILE]
#
# `Layout/LineLength` is the largest number in every layout report tuxedo
# produces, and most of it is not work. A line holding one string, regex or
# comment wider than the limit cannot be brought under it by any placement:
# the only remaining move is to split the literal, which tuxedo does not do
# (scripts/format-verify.rubocop.yml says why). Counting those next to lines
# tuxedo genuinely fails to break hides the second number behind the first.
#
# Every over-long line is measured against its widest *atom* -- a string,
# regex, symbol or backtick literal, a number, or a comment. The best a
# formatter can do is stand that atom alone on a line at the statement's own
# indent; it can nest deeper but never dedent. If the line is still over the
# limit there, nothing is left to try:
#
#   UNSPLITTABLE  indent + widest atom > limit
#   BREAKABLE     everything else -- a construct tuxedo does not break yet
#
# A literal spanning several lines counts on each of them: the lines after
# its opening delimiter are content, and the opening line is pinned from that
# delimiter to its end. Otherwise a `%{...}` carrying a block of SQL would
# read as breakable code on every line of it.
#
# A call written without parentheses counts as one atom with its single
# argument, because there is no comma to break at and tuxedo does not add
# parentheses the author did not write. With several arguments it is
# breakable: Ruby takes a line break after a comma without them.
#
# A hash pair whose value is a literal counts as one atom with it, for the
# same reason: tuxedo does not break after the key, since a value left
# dangling under its key reads worse than a long line.
#
# The split is a triage aid, not a verdict. A BREAKABLE line can still turn
# out to need a layout decision rather than a fix, and those belong in an
# issue, not in this script.
#
# Reads a directory tree, not a file list, and mirrors the matcher in
# src/walk.rs so a Rakefile or .gemspec is measured like everything else.
# Files that do not parse are skipped: they are not tuxedo's to format.

require "prism"
require "set"

# A literal whose interior tuxedo copies verbatim. Splitting one changes the
# program, so its width is a floor on the line that holds it.
ATOMS = [
  Prism::StringNode, Prism::InterpolatedStringNode,
  Prism::RegularExpressionNode, Prism::InterpolatedRegularExpressionNode,
  Prism::SymbolNode, Prism::InterpolatedSymbolNode,
  Prism::XStringNode, Prism::InterpolatedXStringNode,
  Prism::IntegerNode, Prism::FloatNode, Prism::RationalNode, Prism::ImaginaryNode
].freeze

# A path of names with no argument list anywhere in it: `Foo::Bar`,
# `app.config.x`. Tuxedo has nowhere to break inside one either.
def name_path?(node)
  case node
  when Prism::ConstantReadNode, Prism::ConstantPathNode, Prism::LocalVariableReadNode,
       Prism::InstanceVariableReadNode, Prism::SelfNode, Prism::GlobalVariableReadNode
    true
  when Prism::CallNode
    node.arguments.nil? && node.block.nil? && node.opening_loc.nil? &&
      node.name.to_s.match?(/\A[a-z_]\w*[?!]?\z/i) && (node.receiver.nil? || name_path?(node.receiver))
  else
    false
  end
end

MATCHER = %w[*.rb *.rake *.gemspec *.ru Rakefile Gemfile].freeze

# Two exemptions Layout/LineLength applies by default. Without them this
# script counts lines rubocop does not report and the split no longer adds
# up to the number in the layout report it exists to explain.
COP_DIRECTIVE = /\s*#\s*rubocop\s*:\s*(?:disable|enable|todo)\b.*\z/
URI_SCHEME = %r{\b[a-z][a-z0-9+.-]*://\S+}i

# IgnoreCopDirectives: a trailing `# rubocop:disable ...` does not count
# toward the length. AllowURI: a line is allowed to run over when a URI is
# what crosses the limit -- there is nothing to break inside one.
def measured_length(line, limit)
  line = line.sub(COP_DIRECTIVE, "")
  return line.length unless line.length > limit

  line.to_enum(:scan, URI_SCHEME).each do
    match = Regexp.last_match
    return limit if match.begin(0) <= limit && match.end(0) >= limit
  end
  line.length
end

Offense = Struct.new(:path, :line, :width, :best, :atom, :atom_width)

# AllowHeredoc, the third default: the body of a heredoc is content, and its
# width is the author's business rather than the formatter's.
def heredoc_body_lines(node, lines = [])
  return lines unless node.is_a?(Prism::Node)

  opening = node.respond_to?(:opening_loc) ? node.opening_loc : nil
  if opening && opening.slice.start_with?("<<") && node.closing_loc
    lines << ((opening.end_line + 1)...node.closing_loc.start_line)
  end
  node.compact_child_nodes.each { |child| heredoc_body_lines(child, lines) }
  lines
end

# Lines after the first of a multi-line literal are its content: a `%{...}`
# or a quoted string carried over several lines. Nothing on them is code, so
# no placement shortens them -- the single-line atom rule below cannot see
# this, because the atom's own location spans lines.
def literal_body_lines(node, lines = [])
  return lines unless node.is_a?(Prism::Node)

  loc = node.location
  if ATOMS.any? { |k| node.is_a?(k) } && loc.end_line > loc.start_line
    lines << ((loc.start_line + 1)..loc.end_line)
  end
  node.compact_child_nodes.each { |child| literal_body_lines(child, lines) }
  lines
end

# The sole argument of a call written without parentheses cannot be moved to
# a line of its own: there is no comma to break at, and tuxedo does not add
# parentheses the author did not write. So the whole call is as immovable as
# the literal inside it -- `raise "huge message"`, the example in
# scripts/format-verify.rubocop.yml. A bare call with *several* arguments is
# breakable, because Ruby takes a line break after a comma without them.
def bare_call_pinning_an_atom?(node, lineno)
  return false unless node.is_a?(Prism::CallNode) && node.opening_loc.nil?
  return false unless node.location.start_line == lineno

  arguments = node.arguments&.arguments
  return false unless arguments&.size == 1

  pinned_sole_argument?(arguments.first, lineno)
end

# The sole argument a bare call pins: a literal, or another bare call that
# pins one in turn -- `eq "huge string"` in `.to eq "huge string"`, where
# neither call has a comma to break at.
def pinned_sole_argument?(only, lineno)
  only.location.start_line == lineno &&
    (ATOMS.any? { |k| only.is_a?(k) } || bare_call_pinning_an_atom?(only, lineno))
end

# `receiver.to eq "huge string"`, the receiver ending on an earlier line or
# earlier on this one: the chain may break before the `.`, and nowhere after
# it, so from the `.` to the end of the line is one piece.
def bare_link_pinning_an_atom(node, lineno, line)
  return unless node.is_a?(Prism::CallNode) && node.opening_loc.nil? && node.receiver
  return unless (dot = node.call_operator_loc) && dot.start_line == lineno

  arguments = node.arguments&.arguments
  return unless arguments&.size == 1 && pinned_sole_argument?(arguments.first, lineno)

  moved_width(line.length - dot.start_column, dot.start_column, line)
end

# A hash pair whose value is a literal, or a literal by itself: the pieces a
# bare call's first argument can be without there being a place to break.
def pinned_argument?(node)
  return ATOMS.any? { |k| node.is_a?(k) } if !node.is_a?(Prism::AssocNode) && !node.is_a?(Prism::KeywordHashNode)

  pairs = node.is_a?(Prism::KeywordHashNode) ? node.elements.first(1) : [node]
  pairs.all? do |pair|
    pair.is_a?(Prism::AssocNode) &&
      (ATOMS.any? { |k| pair.value.is_a?(k) } || name_path?(pair.value) || call_head_end(pair.value))
  end
end

# The first argument of a call written without parentheses starts on the
# line of the method name: `assert_equal "long string",` cannot put the
# string anywhere but after `assert_equal `. The rest may follow a comma
# onto the next line, so with several arguments it is name plus first
# argument that is pinned.
# What follows an atom on its line and cannot be separated from it: the
# comma, closing bracket, continuation or operator that ends the line.
STICKY_TAIL = /\A(?: ?(?:[,)\]}+\\*%\/<>-]|\|\||&&|do\b|\{))*\z/

def sticky_tail_width(line, end_column)
  tail = line[end_column..].to_s
  tail.match?(STICKY_TAIL) ? tail.length : 0
end

# An atom that does not start its line goes onto a line of its own one
# indent in, so that is the column it has to fit from.
NEST = 2

def moved_width(width, start_column, line)
  start_column > line[/\A */].length ? width + NEST : width
end

def bare_call_pinning_first_argument(node, lineno, line)
  return unless node.is_a?(Prism::CallNode) && node.opening_loc.nil?
  return unless node.receiver.nil? || name_path?(node.receiver)
  return unless node.location.start_line == lineno

  first = node.arguments&.arguments&.first
  first = first.elements.first if first.is_a?(Prism::KeywordHashNode)
  return unless first && pinned_argument?(first)
  return unless first.location.end_line == lineno || (first.is_a?(Prism::AssocNode) && call_head_end(first.value))

  end_column = first.location.end_column
  end_column = call_head_end(first.value) || end_column if first.is_a?(Prism::AssocNode)
  end_column - node.location.start_column + sticky_tail_width(line, end_column)
end

# `Path::To.method(`: where the part of a call that cannot be broken ends.
def call_head_end(node)
  return unless node.is_a?(Prism::CallNode) && node.opening_loc && node.receiver && name_path?(node.receiver)
  return unless node.receiver.location.start_line == node.opening_loc.start_line

  node.opening_loc.end_column
end

# An assignment whose value is an operator chain (`a = b ||\n c`,
# `a = (b |\n c)`): tuxedo leaves the first operand after the `=`, so the
# line up to the first operator is pinned.
def chain_value?(value)
  case value
  when Prism::AndNode, Prism::OrNode
    true
  when Prism::ParenthesesNode
    value.body.is_a?(Prism::StatementsNode) && value.body.body.size == 1 && chain_value?(value.body.body.first)
  when Prism::CallNode
    (value.receiver && value.opening_loc.nil? && value.arguments&.arguments&.size == 1 &&
      value.name.to_s.match?(/\A[^a-z_]/i)) || (value.receiver && chain_value?(value.receiver))

  else
    false
  end
end

def write_value(node)
  if node.class.name.end_with?("WriteNode") && node.respond_to?(:value)
    node.value
  elsif node.is_a?(Prism::CallNode) && node.name.to_s.end_with?("=") && node.opening_loc.nil? &&
        node.arguments&.arguments&.size == 1
    node.arguments.arguments.first
  end
end

# Whether an implicit concatenation (`"a" "b"`, `"a" \\`) starts on this line.
def implicit_concatenation_at?(node, lineno)
  return false unless node.is_a?(Prism::Node)
  return true if node.is_a?(Prism::InterpolatedStringNode) && node.opening_loc.nil? &&
                 node.location.start_line == lineno

  node.compact_child_nodes.any? { |child| implicit_concatenation_at?(child, lineno) }
end

def widest_atom(node, lineno, line_length, best, line = "")
  return best unless node.is_a?(Prism::Node)

  loc = node.location
  if ATOMS.any? { |k| node.is_a?(k) } && loc.start_line == lineno
    # A literal that starts here and ends further down still pins everything
    # from its opening delimiter to the end of this line.
    width = loc.end_line == lineno ? loc.end_column - loc.start_column + sticky_tail_width(line, loc.end_column) : line_length - loc.start_column
    width = moved_width(width, loc.start_column, line)
    best = [width, node.class.name.split("::").last] if width > best[0]
  end

  # A hash pair is as immovable as its value: tuxedo does not break after
  # the key, so the key and the literal after it are one atom.
  if node.is_a?(Prism::AssocNode) && loc.start_line == lineno &&
     (ATOMS.any? { |k| node.value.is_a?(k) } || name_path?(node.value)) && node.value.location.end_line == lineno
    width = node.value.location.end_column - loc.start_column + sticky_tail_width(line, node.value.location.end_column)
    width = moved_width(width, loc.start_column, line)
    best = [width, "HashPair"] if width > best[0]
  end

  # `Path::To.method(`: the head in front of an argument list has no place
  # to break; tuxedo moves the whole call under an assignment target when
  # the head does not fit, and cannot do more.
  if node.is_a?(Prism::CallNode) && node.opening_loc && node.opening_loc.start_line == lineno &&
     node.receiver && name_path?(node.receiver) && node.receiver.location.start_line == lineno
    width = node.opening_loc.end_column - node.receiver.location.start_column
    width = moved_width(width, node.receiver.location.start_column, line)
    best = [width, "CallHead"] if width > best[0]
  end

  # The value of an assignment that is an operator chain: only the chain's
  # own operators are places to break, so the first line is pinned.
  if node.location.start_line == lineno && (value = write_value(node)) && chain_value?(value) &&
     value.location.start_line == lineno && value.location.end_line > lineno
    best = [line.length - line[/\A */].length, "AssignedChain"] if line.length - line[/\A */].length > best[0]
  end

  # The value of an assignment that is a command call with several
  # arguments (`x = stub_request "a" => 1,\n "b" => 2`): only its commas are
  # places to break, so the first line is pinned.
  if node.location.start_line == lineno && (value = write_value(node)) && value.is_a?(Prism::CallNode) &&
     value.opening_loc.nil? && value.receiver.nil? && value.location.start_line == lineno &&
     value.location.end_line > lineno && (value.arguments&.arguments&.size.to_i > 1 ||
       value.arguments&.arguments&.first.is_a?(Prism::KeywordHashNode))
    best = [line.length - line[/\A */].length, "AssignedCommand"] if line.length - line[/\A */].length > best[0]
  end

  # Adjacent literals (`"a" "b"`, `"a" \\`): the first fragment stays where
  # it starts, after whatever precedes it.
  if node.is_a?(Prism::InterpolatedStringNode) && node.opening_loc.nil? && loc.start_line == lineno &&
     (first = node.parts.first) && first.location.end_line == lineno
    width = first.location.end_column - loc.start_column + sticky_tail_width(line, first.location.end_column)
    width = moved_width(width, loc.start_column, line)
    best = [width, "ImplicitConcatenation"] if width > best[0]
  end

  # A class header is one line: `class Name < Superclass`.
  if node.is_a?(Prism::ClassNode) && loc.start_line == lineno && (superclass = node.superclass) &&
     superclass.location.end_line == lineno
    width = superclass.location.end_column - loc.start_column
    best = [width, "ClassHeader"] if width > best[0]
  end

  # A method name is one token, and `def name` cannot be split from it.
  if node.is_a?(Prism::DefNode) && loc.start_line == lineno && node.name_loc.end_line == lineno
    width = node.name_loc.end_column - loc.start_column
    best = [width, "MethodName"] if width > best[0]
  end

  # A parenthesised literal is one piece: `(+"...").force_encoding(`.
  if node.is_a?(Prism::ParenthesesNode) && loc.start_line == lineno && loc.end_line == lineno &&
     node.body.is_a?(Prism::StatementsNode) && node.body.body.size == 1 &&
     (ATOMS.any? { |k| node.body.body.first.is_a?(k) } ||
      (node.body.body.first.is_a?(Prism::CallNode) && node.body.body.first.name.to_s.end_with?("@")))
    width = loc.end_column - loc.start_column
    best = [width, "ParenthesisedLiteral"] if width > best[0]
  end

  # A pattern is held on one line (`in [a, b]`, `x in {k: v}`): the layout
  # has no expanded shape for it.
  if [Prism::ArrayPatternNode, Prism::HashPatternNode, Prism::FindPatternNode].any? { |k| node.is_a?(k) } &&
     loc.start_line == lineno && loc.end_line == lineno
    width = loc.end_column - loc.start_column
    best = [width, "Pattern"] if width > best[0]
  end
  if (node.is_a?(Prism::MatchPredicateNode) || node.is_a?(Prism::MatchRequiredNode) || node.is_a?(Prism::InNode)) &&
     loc.start_line == lineno && node.pattern.location.end_line == lineno &&
     [Prism::ArrayPatternNode, Prism::HashPatternNode, Prism::FindPatternNode].any? { |k| node.pattern.is_a?(k) }
    width = node.pattern.location.end_column - loc.start_column
    best = [width, "Pattern"] if width > best[0]
  end

  # Block parameters are never wrapped, so the call and its `do |...|` are
  # one line.
  if node.is_a?(Prism::BlockNode) && node.parameters && node.opening_loc.start_line == lineno &&
     node.parameters.location.end_line == lineno
    width = node.parameters.location.end_column - (node.location.start_line == lineno ? 0 : loc.start_column)
    best = [width, "BlockParameters"] if width > best[0]
  end

  if (width = bare_call_pinning_first_argument(node, lineno, line))
    best = [width, "BareCallArgument"] if width > best[0]
  end

  if bare_call_pinning_an_atom?(node, lineno)
    width = line_length - loc.start_column
    best = [width, "BareCallArgument"] if width > best[0]
  end

  if (width = bare_link_pinning_an_atom(node, lineno, line))
    best = [width, "BareCallArgument"] if width > best[0]
  end

  node.compact_child_nodes.each { |child| best = widest_atom(child, lineno, line_length, best, line) }
  best
end

dir = ARGV.shift or abort "Usage: scripts/classify-linelength.rb <dir> [--limit N] [--list N] [--sites FILE]"
limit = (ARGV[ARGV.index("--limit") + 1].to_i if ARGV.include?("--limit")) || 120
list = (ARGV[ARGV.index("--list") + 1].to_i if ARGV.include?("--list")) || 0
sites = (ARGV[ARGV.index("--sites") + 1] if ARGV.include?("--sites"))

# With --sites, classify exactly the lines rubocop reported ("path:line" per
# line, relative to <dir>) so the two numbers add up to the one in the layout
# report. Without it, find the over-long lines here: cheaper and needs no
# rubocop, at the cost of a small disagreement with its count -- the
# exemptions below are reimplementations, not the originals.
wanted = nil
if sites
  wanted = File.readlines(sites, chomp: true).reject(&:empty?).group_by { |s| s.split(":").first }
  files = wanted.keys.map { |rel| File.join(dir, rel) }
else
  files = MATCHER.flat_map { |glob| Dir.glob(File.join(dir, "**", glob)) }.sort.uniq
end

offenses = []

files.each do |path|
  source = File.read(path)
  rel = path.delete_prefix("#{dir}/")
  on_line = wanted && wanted.fetch(rel, []).map { |s| s.split(":").last.to_i }.to_set
  next unless on_line || source.lines.any? { |l| measured_length(l.chomp, limit) > limit }

  result = Prism.parse(source)
  next if result.failure?

  # One pass over the comments; the tree is walked per offending line, which
  # is cheap because only a handful of lines in a file are ever over.
  comments = result.comments.group_by { |c| c.location.start_line }
  heredocs = heredoc_body_lines(result.value)
  literals = literal_body_lines(result.value)
  # A `=begin`/`=end` block is one comment to Prism, starting at `=begin`:
  # grouped by start line it would leave every line inside it looking like
  # code with nothing in it to measure.
  embdocs = result.comments.grep(Prism::EmbDocComment).map { |c| c.location.start_line..c.location.end_line }

  source.lines.each_with_index do |line, index|
    line = line.chomp
    lineno = index + 1

    if on_line
      next unless on_line.include?(lineno)
    else
      next unless measured_length(line, limit) > limit
      next if heredocs.any? { |range| range.cover?(lineno) }
    end

    if embdocs.any? { |range| range.cover?(lineno) }
      # The author's prose, which tuxedo copies as it is.
      offenses << Offense.new(rel, lineno, line.length, line.length, "EmbeddedDocument", line.length)
      next
    end

    if literals.any? { |range| range.cover?(lineno) }
      # Content, not code: report it at its own width so it sorts sensibly
      # and lands in UNSPLITTABLE whatever the indent is.
      offenses << Offense.new(rel, lineno, line.length, line.length, "LiteralBody", line.length)
      next
    end

    width, atom = widest_atom(result.value, lineno, line.length, [0, nil], line)
    trailing = comments.fetch(lineno, []).find { |c| !line[0...c.location.start_column].strip.empty? }
    if trailing && line[0...trailing.location.start_column].rstrip.length <= limit
      # Tuxedo leaves a comment on the line it was written on, so the code
      # before it cannot be given a line of its own: what is over the limit
      # is the comment, and moving that is not a layout decision.
      offenses << Offense.new(rel, lineno, line.length, limit + 1, "TrailingComment", line.length)
      next
    end
    atom = "ImplicitConcatenation" if implicit_concatenation_at?(result.value, lineno)
    comments.fetch(lineno, []).each do |comment|
      cw = comment.location.end_column - comment.location.start_column
      width, atom = cw, "Comment" if cw > width
    end

    indent = line[/\A */].length
    offenses << Offense.new(rel, lineno, line.length, indent + width, atom || "none", width)
  end
end

# Kinds tuxedo leaves alone whatever the room: the first fragment of an
# implicit concatenation stays after the `=` it follows, because moving it
# would leave its continuation lines at an indent nothing else uses.
ALWAYS_PINNED = %w[ImplicitConcatenation].freeze
unsplittable, breakable = offenses.partition { |o| o.best > limit || ALWAYS_PINNED.include?(o.atom) }

# In --sites mode the caller already knows the total and is relying on this
# split to account for it, so a site that produced no verdict -- a file
# Prism rejects though rubocop read it, a line number past the end -- is
# reported rather than quietly dropped.
unclassified = wanted ? wanted.values.sum(&:size) - offenses.size : 0

if list.positive?
  [["UNSPLITTABLE", unsplittable], ["BREAKABLE", breakable]].each do |label, group|
    next if group.empty?

    puts "#{label} (#{group.size})"
    group.sort_by { |o| [-o.width, o.path] }.first(list).each do |o|
      puts format("  %-58s %4d cols, best %4d (%s %d)",
                  "#{o.path}:#{o.line}", o.width, o.best, o.atom, o.atom_width)
    end
    puts
  end
end

summary = "linelength: #{offenses.size + unclassified} over #{limit}, " \
          "#{unsplittable.size} unsplittable, #{breakable.size} breakable"
summary += ", #{unclassified} unclassified" if unclassified.positive?
puts summary
