module HankoXml
  class Error < StandardError; end
  class ConfigurationError < Error; end
  class NativeVersionMismatch < Error; end # :nodoc:

  module Extensions; end

  class << self; end

  def reset; end
  def self.configure(options = {}); end

  # A comment inside keeps the body.
  def documented
    # Overridden by subclasses.
  end

  def with_trailing_comment # :nodoc:
  end

  class ThisErrorNameIsLongEnoughThatTheWholeOneLineDeclarationDoesNotFit < HankoXml::Namespaced::ConfigurationError
  end
end
