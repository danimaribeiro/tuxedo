# frozen_string_literal: true
#
# Rake tasks to build and package native `tuxedo-r` gems with a compiled
# `tuxedo` binary for each supported platform.
#
# TL;DR: run "rake package"
#
# Each native platform gem (see Tuxedo::R::NativePlatforms::TARGETS) contains
# everything the vanilla gem does, plus:
#
#   exe/
#   ├── tuxedo                        # generic ruby script that locates + execs the binary
#   └── <rubygems platform>/
#       └── tuxedo                    # the compiled tuxedo binary for that platform
#
# `exe/tuxedo` is what actually gets installed onto the user's PATH (RubyGems
# requires gem executables to be Ruby scripts); it shells out to the binary
# living alongside it under `exe/<platform>/`.
#
# Binaries are built from the sibling Rust crate at ../ (i.e. tuxedo/Cargo.toml)
# using `cargo build --release --target <triple>` for the host target, or
# `cross build` for cross-compiled targets. Set CROSS=1 to force use of `cross`
# for every target (useful in CI, where a single Linux runner cross-compiles
# for all targets via Docker).
#
# New rake tasks created:
#
#   rake gem:ruby             # Build the platform-independent "ruby" gem (no binary)
#   rake gem:<platform>       # Build a single native platform gem, e.g. rake gem:arm64-darwin
#   rake build:<platform>     # Compile just the tuxedo binary for one platform
#   rake build:all            # Compile the tuxedo binary for every supported platform
#
# Modified rake tasks:
#
#   rake gem / package / repackage  # Now build every platform gem, not just one
#
require "rubygems/package_task"
require "fileutils"
require_relative "../lib/tuxedo/r/native_platforms"

TUXEDO_R_GEMSPEC = Bundler.load_gemspec("tuxedo-r.gemspec")
TUXEDO_CRATE_DIR = File.expand_path("../..", __dir__) # tuxedo/

gem_path = Gem::PackageTask.new(TUXEDO_R_GEMSPEC).define
desc "Build the platform-independent ruby gem (no binary, for reference/testing only)"
task "gem:ruby" => [gem_path]

exepaths = []

Tuxedo::R::NativePlatforms::TARGETS.each do |platform, target_triple|
  binary_name = Tuxedo::R::NativePlatforms.binary_name(platform)

  TUXEDO_R_GEMSPEC.dup.tap do |gemspec|
    exedir = File.join(gemspec.bindir, platform) # "exe/x86_64-linux"
    exepath = File.join(exedir, binary_name) # "exe/x86_64-linux/tuxedo"
    exepaths << exepath

    gemspec.platform = platform
    gemspec.files += [exepath]

    gem_path = Gem::PackageTask.new(gemspec).define
    desc "Build the #{platform} native gem"
    task "gem:#{platform}" => [gem_path]

    directory exedir

    desc "Compile the tuxedo binary for #{platform} (#{target_triple})"
    task "build:#{platform}" => [exedir] do
      built_path = File.join(TUXEDO_CRATE_DIR, "target", target_triple, "release", binary_name)

      cmd = if ENV["CROSS"] == "1" || !target_triple.include?(RbConfig::CONFIG["host_cpu"])
        ["cross", "build", "--release", "--target", target_triple]
      else
        ["cargo", "build", "--release", "--target", target_triple]
      end

      warn "Building #{exepath} via `#{cmd.join(" ")}` ..."
      sh(*cmd, chdir: TUXEDO_CRATE_DIR)

      FileUtils.cp(built_path, exepath, verbose: true)
      FileUtils.chmod(0o755, exepath, verbose: true)
    end

    file exepath => "build:#{platform}"
  end
end

desc "Compile the tuxedo binary for every supported platform"
task "build:all" => Tuxedo::R::NativePlatforms::TARGETS.keys.map { |p| "build:#{p}" }

CLOBBER.add(exepaths.map { |p| File.dirname(p) })
