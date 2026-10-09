#!/usr/bin/env ruby
# frozen_string_literal: true

# Splits the offenses RuboCop reports on tuxedo's output, and the comments the
# placement check reports as displaced, into the ones recorded as decisions in
# scripts/verify-accepted.tsv and the ones nobody has explained.
#
# Usage:
#   scripts/classify-offenses.rb <formatted-dir> --accepted FILE
#       [--offenses EMACS_FILE] [--comments COMMENT_ANCHORS_OUTPUT] [--project NAME]
#
# --offenses is `rubocop --format emacs` output; Layout/LineLength is left to
# scripts/classify-linelength.rb. The last line printed is machine-read by
# format-verify.sh.

require "prism"
require "set"

dir = ARGV.shift or abort "usage: classify-offenses.rb <formatted-dir> --accepted FILE [--offenses FILE] [--comments FILE] [--project NAME]"

def option(name)
  ARGV[ARGV.index(name) + 1] if ARGV.include?(name)
end

accepted_path = option("--accepted") or abort "--accepted is required"
offenses_path = option("--offenses")
comments_path = option("--comments")
project = option("--project")

Rule = Struct.new(:scope, :before, :pattern, :reason, :witness, :matched) do
  # Identity, not value: `matched` changes as the rule is counted.
  def hash = object_id.hash
  def eql?(other) = equal?(other)
end
rules = File.readlines(accepted_path, chomp: true).reject { |l| l.empty? || l.start_with?("#") }.map do |line|
  scope, before, pattern, reason, witness = line.split("\t", 5)
  Rule.new(scope, before.to_i, pattern, reason, witness, 0)
end

def heredoc_lines(path)
  result = Prism.parse_file(path)
  ranges = []
  stack = [result.value]
  until stack.empty?
    node = stack.pop
    next unless node.is_a?(Prism::Node)

    if node.respond_to?(:opening_loc) && node.opening_loc&.slice&.start_with?("<<") && node.closing_loc
      ranges << ((node.opening_loc.end_line + 1)...node.closing_loc.start_line)
    end
    stack.concat(node.compact_child_nodes)
  end
  ranges
end

heredocs = Hash.new { |h, path| h[path] = heredoc_lines(path) }
lines_of = Hash.new { |h, path| h[path] = File.readlines(path, chomp: true) }

matches = lambda do |rule, text, path, lineno|
  if rule.pattern == "@heredoc"
    heredocs[path].any? { |range| range.cover?(lineno) }
  else
    # A file tuxedo refuses for not being valid UTF-8 is still read here
    # (rubocop can lint text a Ruby string can't hold), and `scrub` keeps a
    # rule that matches its magic comment from raising on the invalid bytes
    # elsewhere in its window instead of just not matching them.
    Regexp.new(rule.pattern, Regexp::MULTILINE).match?(text.scrub)
  end
end

Finding = Struct.new(:where, :scope, :message, :path, :lineno, :window)
findings = []

if offenses_path
  File.foreach(offenses_path, chomp: true) do |line|
    next unless (m = line.match(/\A(.+?):(\d+):\d+: \w: (?:\[\w+\] )?([A-Za-z]+\/\w+): (.*)\z/))

    rel, lineno, cop, message = m[1], m[2].to_i, m[3], m[4]
    next if cop == "Layout/LineLength"

    rel = rel.sub(%r{\A#{Regexp.escape(dir)}/}, "")
    path = File.join(dir, rel)
    lines = lines_of[path]
    # An alignment offense names the line of what it wanted the closer
    # under (`end` at 120, 12 is not aligned with `case` at 115, 19), which
    # may sit any distance above: the rule's window ends there instead.
    anchor = message[/ is not aligned with .* at (\d+), \d+\.?\z/, 1]&.to_i || lineno
    window = ->(before) { lines[[anchor - 1 - before, 0].max..(anchor - 1)].join("\n") }
    findings << Finding.new("#{rel}:#{lineno}", cop, message, path, lineno, window)
  end
end

if comments_path
  File.foreach(comments_path, chomp: true) do |line|
    next unless (m = line.match(/\A\s*DISPLACED: (\S+?):(\d+) (.*)\z/))

    findings << Finding.new("#{m[1]}:#{m[2]}", "Comments", m[3], nil, m[2].to_i, ->(_) { m[3] })
  end
end

accepted = Hash.new { |h, k| h[k] = 0 }
unexplained = []
findings.each do |f|
  rule = rules.find do |r|
    r.scope == f.scope && matches.call(r, f.window.call(r.before), f.path, f.lineno)
  end
  if rule
    rule.matched += 1
    accepted[rule] += 1
  else
    unexplained << f
  end
end

by_scope = accepted.group_by { |rule, _| rule.scope }
by_scope.sort.each do |scope, group|
  group.each do |rule, count|
    puts format("  accepted %4d  %-40s %s", count, scope, rule.reason)
  end
end
unexplained.each { |f| puts "  UNEXPLAINED #{f.where} #{f.scope}: #{f.message}" }
stale_rules = rules.select { |r| r.matched.zero? && r.witness == project }
stale_rules.each do |r|
  puts "  STALE #{r.scope} /#{r.pattern}/ matches nothing: #{r.reason}"
end

layout_unexplained = unexplained.count { |f| f.scope != "Comments" }
comments_unexplained = unexplained.count { |f| f.scope == "Comments" }
stale = stale_rules.size
puts "offenses: #{findings.size - findings.count { |f| f.scope == 'Comments' }} listed, " \
     "#{layout_unexplained} unexplained layout, #{comments_unexplained} unexplained comments, #{stale} stale"
