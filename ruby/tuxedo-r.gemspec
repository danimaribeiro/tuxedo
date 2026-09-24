# frozen_string_literal: true

require_relative "lib/tuxedo/r/version"

Gem::Specification.new do |spec|
  spec.name = "tuxedo-r"
  spec.version = Tuxedo::R::VERSION
  spec.authors = ["Danimar Ribeiro"]
  spec.summary = "A self-contained `tuxedo` executable: a fast Ruby formatter written in Rust."
  spec.description = <<~DESC
    Tuxedo is a Ruby code formatter written in Rust, distributed as a
    precompiled native binary wrapped in this Ruby gem (the "R" is both Ruby and
    Rust). It is `bundle exec`-compatible, and a linter is planned.
  DESC
  spec.homepage = "https://github.com/danimaribeiro/tuxedo"
  spec.license = "MIT"
  spec.required_ruby_version = ">= 3.0.0"

  spec.metadata["source_code_uri"] = spec.homepage
  spec.metadata["changelog_uri"] = "#{spec.homepage}/blob/main/CHANGELOG.md"
  spec.metadata["rubygems_mfa_required"] = "true"

  # NOTE: the platform-specific gems built by rakelib/package.rake add the
  # compiled `exe/<platform>/tuxedo` binary on top of this file list.
  spec.files = Dir["lib/**/*", "exe/tuxedo", "README.md", "LICENSE"]
  spec.bindir = "exe"
  spec.executables << "tuxedo"
  spec.require_paths = ["lib"]
end
