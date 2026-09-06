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
