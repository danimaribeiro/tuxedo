x = a + b
y = a <=> b

def foo
  unless ENV["X"]
    warn "Deprecation warning: #{message}\n" +
      "You can silence deprecations warning by setting the environment variable THOR_SILENCE_DEPRECATION."
  end
end

(foreign_key_for?(record) && record_foreign_key_matches_owner?(record)) ||
  (foreign_key_for?(owner) && owner_foreign_key_matches_record?(record))

# `a + b + c` is nested call nodes, and one group per node would bin-pack --
# the whole spine is one group, so either it fits or every operand gets its
# own line.
def recipients
  Array(to) +
    Array(cc) +
    Array(bcc) +
    Array(header[:x_original_to]).map(&:to_s) +
    Array(header[:x_forwarded_to]).map(&:to_s)
end

def validate_reference(errors, ref)
  if external?(ref)
    errors << HankoXml::PolicyError.new(
      "reference URI #{ref.uri.inspect} is external, and allow_external_references is false"
    )
    error = HankoXml::PolicyError.new(
      "reference URI #{ref.uri.inspect} is external, and allow_external_references is false"
    )
    errors << HankoXml::PolicyError.new(
      "reference URI #{ref.uri.inspect} is external, " \
        "and allow_external_references is false"
    )
    rows << [
      first_column_value,
      second_column_value,
      third_column_value,
      fourth_column_value,
      fifth_column_value,
      sixth
    ]
  end
end

# A spine ending in a call with a block keeps the block on the operator's line
# while the head fits, and gives it a continuation line of its own when it
# does not, with the body a level in from that line.
def bottle_matches?(bottle, spec, formula, root_url)
  short && list.any? do |x|
    x.y
  end
end

def metadata_matches?(bottle, spec, formula, root_url)
  bottle.fetch("formula").fetch("pkg_version") == formula.pkg_version.to_s &&
    bottle.fetch("rebuild", 0).to_i == spec.rebuild &&
    (GitHubPackages.root_url_if_match(root_url) || root_url) == spec.root_url &&
    bottle.fetch("tags").all? do |tag, tag_hash|
      tag_spec = spec.collector.specification_for(Utils::Bottles.tag(tag.to_sym), no_older_versions: true)
      tag_spec && tag_spec.checksum.hexdigest == tag_hash.fetch("sha256")
    end
end

def macos_dependency?(argument)
  literal_value(argument) == :macos ||
    (argument.hash_type? && T.cast(argument, RuboCop::AST::HashNode).pairs.any? do |pair|
      literal_value(pair.key) == :macos
    end)
end

# A wrapped `return` value continues two levels in, like a block condition.
def compact_blank?(arguments, receiver)
  return use_single_value_block_argument?(arguments, receiver_in_block) ||
      use_hash_value_block_argument?(arguments, receiver_in_block)
end
