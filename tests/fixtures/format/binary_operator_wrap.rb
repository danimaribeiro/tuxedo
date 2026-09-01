x = a + b
y = a <=> b

def foo
  unless ENV["X"]
    warn "Deprecation warning: #{message}\n" +
      "You can silence deprecations warning by setting the environment variable THOR_SILENCE_DEPRECATION."
  end
end
