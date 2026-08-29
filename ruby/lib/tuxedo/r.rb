# frozen_string_literal: true

require_relative "r/version"
require_relative "r/native_platforms"

module Tuxedo
  # The "R" is deliberately ambiguous: this is the Ruby wrapper around a
  # Rust-compiled `tuxedo` binary.
  module R
    DEFAULT_DIR = File.expand_path(File.join(__dir__, "..", "..", "exe"))
    GEM_NAME = "tuxedo-r"

    # Raised when the host platform has no precompiled tuxedo binary published.
    class UnsupportedPlatformException < StandardError
    end

    # Raised when the tuxedo executable could not be found where expected.
    class ExecutableNotFoundException < StandardError
    end

    # Raised when TUXEDO_INSTALL_DIR is set but does not exist.
    class DirectoryNotFoundException < StandardError
    end

    class << self
      # e.g. "x86_64-linux"
      def platform
        [:cpu, :os].map { |m| Gem::Platform.local.send(m) }.join("-")
      end

      # Absolute path to the tuxedo binary that should be executed on this host.
      #
      # Resolution order:
      #   1. TUXEDO_INSTALL_DIR env var, if set (escape hatch for local dev / CI
      #      builds that want to use a freshly-built binary instead of the one
      #      vendored into the installed platform gem).
      #   2. The platform-specific binary bundled into the matching
      #      tuxedo-r-<platform> gem.
      def executable(exe_path: DEFAULT_DIR)
        tuxedo_install_dir = ENV["TUXEDO_INSTALL_DIR"]

        if tuxedo_install_dir
          unless File.directory?(tuxedo_install_dir)
            raise DirectoryNotFoundException, <<~MESSAGE
              TUXEDO_INSTALL_DIR is set to #{tuxedo_install_dir}, but that directory does not exist.
            MESSAGE
          end

          warn "NOTE: using TUXEDO_INSTALL_DIR to find the tuxedo executable: #{tuxedo_install_dir}"
          exe_file = File.expand_path(File.join(tuxedo_install_dir, "tuxedo"))
        else
          if NativePlatforms::TARGETS.keys.none? { |p| Gem::Platform.match_gem?(Gem::Platform.new(p), GEM_NAME) }
            raise UnsupportedPlatformException, <<~MESSAGE
              #{GEM_NAME} does not have a precompiled binary for the #{platform} platform.
              See https://github.com/danimaribeiro/tuxedo#unsupported-platforms for options.
            MESSAGE
          end

          exe_file = Dir.glob(File.expand_path(File.join(exe_path, "*", "tuxedo{,.exe}"))).find do |f|
            Gem::Platform.match_gem?(Gem::Platform.new(File.basename(File.dirname(f))), GEM_NAME)
          end
        end

        if exe_file.nil? || !File.exist?(exe_file)
          raise ExecutableNotFoundException, <<~MESSAGE
            Cannot find the tuxedo executable for #{platform} in #{exe_path}.

            If you're using Bundler, make sure your lockfile includes this platform:

                bundle lock --add-platform #{platform}
                bundle install

            If you're still seeing this after that, check whether
            `bundle config force_ruby_platform` is set to `true`.
          MESSAGE
        end

        exe_file
      end
    end
  end
end
