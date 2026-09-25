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
