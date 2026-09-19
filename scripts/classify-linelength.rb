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
# regex, symbol or backtick literal, or a comment. The best a formatter can
# do is stand that atom alone on a line at the statement's own indent; it can
# nest deeper but never dedent. If the line is still over the limit there,
# nothing is left to try:
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
  Prism::SymbolNode, Prism::XStringNode
].freeze

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

  only = arguments.first
  ATOMS.any? { |k| only.is_a?(k) } && only.location.start_line == lineno
end

def widest_atom(node, lineno, line_length, best)
  return best unless node.is_a?(Prism::Node)

  loc = node.location
  if ATOMS.any? { |k| node.is_a?(k) } && loc.start_line == lineno
    # A literal that starts here and ends further down still pins everything
    # from its opening delimiter to the end of this line.
    width = loc.end_line == lineno ? loc.end_column - loc.start_column : line_length - loc.start_column
    best = [width, node.class.name.split("::").last] if width > best[0]
  end

  # A hash pair is as immovable as its value: tuxedo does not break after
  # the key, so the key and the literal after it are one atom.
  if node.is_a?(Prism::AssocNode) && loc.start_line == lineno &&
     ATOMS.any? { |k| node.value.is_a?(k) } && node.value.location.end_line == lineno
    width = node.value.location.end_column - loc.start_column
    best = [width, "HashPair"] if width > best[0]
  end

  if bare_call_pinning_an_atom?(node, lineno)
    width = line_length - loc.start_column
    best = [width, "BareCallArgument"] if width > best[0]
  end

  node.compact_child_nodes.each { |child| best = widest_atom(child, lineno, line_length, best) }
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

  source.lines.each_with_index do |line, index|
    line = line.chomp
    lineno = index + 1

    if on_line
      next unless on_line.include?(lineno)
    else
      next unless measured_length(line, limit) > limit
      next if heredocs.any? { |range| range.cover?(lineno) }
    end

    if literals.any? { |range| range.cover?(lineno) }
      # Content, not code: report it at its own width so it sorts sensibly
      # and lands in UNSPLITTABLE whatever the indent is.
      offenses << Offense.new(rel, lineno, line.length, line.length, "LiteralBody", line.length)
      next
    end

    width, atom = widest_atom(result.value, lineno, line.length, [0, nil])
    comments.fetch(lineno, []).each do |comment|
      cw = comment.location.end_column - comment.location.start_column
      width, atom = cw, "Comment" if cw > width
    end

    indent = line[/\A */].length
    offenses << Offense.new(rel, lineno, line.length, indent + width, atom || "none", width)
  end
end

unsplittable, breakable = offenses.partition { |o| o.best > limit }

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
