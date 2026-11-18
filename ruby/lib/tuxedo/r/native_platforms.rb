# frozen_string_literal: true

module Tuxedo
  module R
    module NativePlatforms
      # rubygems platform name => rust target triple
      #
      # Keep this in sync with the build and build-windows jobs in
      # tuxedo/.github/workflows/release.yml
      TARGETS = {
        "arm64-darwin" => "aarch64-apple-darwin",
        "x86_64-darwin" => "x86_64-apple-darwin",
        "x86_64-linux" => "x86_64-unknown-linux-gnu",
        "aarch64-linux" => "aarch64-unknown-linux-gnu",
        "x86_64-linux-musl" => "x86_64-unknown-linux-musl",
        "aarch64-linux-musl" => "aarch64-unknown-linux-musl",
        "x64-mingw-ucrt" => "x86_64-pc-windows-msvc",
      }.freeze

      def self.binary_name(platform)
        platform.to_s.include?("mingw") ? "tuxedo.exe" : "tuxedo"
      end
    end
  end
end
