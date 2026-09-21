#!/usr/bin/env ruby
# frozen_string_literal: true

# Measures whether comments stay attached to the code they describe.
#
# Usage:
#   scripts/comment-anchors.rb <source-dir> <formatted-dir> [--list N]
#
# tuxedo's own AST check guarantees no comment is lost. It says nothing
# about *where* a comment ends up, and a comment that moves away from its
# code is worse than no comment: `foo(1, # retries` turning into
# `foo(1, 2)` / `# retries` on the next line now documents nothing.
#
# Every comment has an anchor, read off Prism's token stream:
#
#   - a trailing comment (code before it on its line) is anchored to the
#     last token before it, ignoring a separating comma: in `1, # one` the
#     comment is about `1`.
#   - an own-line comment is anchored to the first token after it, skipping
#     blank lines and further comments: a doc comment is about what follows.
#     One with nothing after it but a closer (`end`, `]`, `}`, `)`) or the end
#     of the file is "dangling", anchored to that closer.
#
# A comment is displaced when its (kind, text, anchor) triple exists in the
# source and not in the formatted output. One exception: an own-line comment
# above `x if cond` that the formatter turned into the block form
# (`if cond` / `x` / `end`) is still directly above the same statement, but
# its anchor is now `if`, so that anchor is accepted for it too. Whitespace, line breaks and the
# `#foo` -> `# foo` normalization never change a triple, so reformatting
# code around a comment is free; only moving it is counted.

require "prism"

Anchor = Struct.new(:kind, :text, :anchor, :line)

SKIPPED_BEFORE = %i[COMMA].freeze
SKIPPED_AFTER = %i[NEWLINE IGNORED_NEWLINE COMMENT EMBDOC_BEGIN EMBDOC_LINE EMBDOC_END].freeze

def normalize(text)
  text = text.rstrip
  return text unless text.start_with?("#") && text.length > 1
  return text if text.start_with?("#!", "##") || text[1].match?(/\s/)

  "# #{text[1..]}"
end

def anchors(path)
  result = Prism.lex(File.read(path))
  return nil unless result.success?

  tokens = result.value.map(&:first)
  tokens.each_with_index.filter_map do |token, index|
    next unless token.type == :COMMENT

    line = token.location.start_line
    before = tokens[0...index].reverse.find { |t| !SKIPPED_BEFORE.include?(t.type) }
    trailing = before && before.location.end_line == line &&
      !%i[NEWLINE IGNORED_NEWLINE COMMENT].include?(before.type)

    if trailing
      Anchor.new(:trailing, normalize(token.value), before.value, line)
    else
      after = tokens[(index + 1)..].find { |t| !SKIPPED_AFTER.include?(t.type) }
      value = after.nil? || after.type == :EOF ? "<eof>" : after.value
      Anchor.new(:leading, normalize(token.value), value, line)
    end
  end
end

source_dir, formatted_dir = ARGV.reject { |a| a.start_with?("--") }.first(2)
list = ARGV.include?("--list") ? ARGV[ARGV.index("--list") + 1].to_i : 0
abort "usage: comment-anchors.rb <source-dir> <formatted-dir> [--list N]" unless source_dir && formatted_dir

total = 0
displaced = []
# Mirrors the matcher in src/walk.rs: every file tuxedo formats, not just
# the ones named *.rb.
MODIFIER_BLOCK_ANCHORS = %w[if unless].freeze
RUBY_GLOBS = ["**/*.rb", "**/*.rake", "**/*.gemspec", "**/*.ru", "**/Rakefile", "**/Gemfile"].freeze

RUBY_GLOBS.flat_map { |glob| Dir.glob(glob, base: source_dir) }.uniq.sort.each do |rel|
  formatted_path = File.join(formatted_dir, rel)
  next unless File.file?(formatted_path)

  before = anchors(File.join(source_dir, rel))
  after = anchors(formatted_path)
  next unless before && after

  total += before.size
  remaining = after.map { |a| [a.kind, a.text, a.anchor] }.tally
  before.each do |a|
    key = [a.kind, a.text, a.anchor]
    if remaining.fetch(key, 0).positive?
      remaining[key] -= 1
    elsif a.kind == :leading && (block_form = MODIFIER_BLOCK_ANCHORS.find { |k| remaining.fetch([a.kind, a.text, k], 0).positive? })
      remaining[[a.kind, a.text, block_form]] -= 1
    else
      displaced << [rel, a]
    end
  end
end

displaced.first(list).each do |rel, a|
  puts "  DISPLACED: #{rel}:#{a.line} #{a.kind} of `#{a.anchor}`: #{a.text[0, 60]}"
end
puts "comments: #{total} total, #{displaced.size} displaced"
