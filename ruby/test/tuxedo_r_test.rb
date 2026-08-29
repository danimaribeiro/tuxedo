# frozen_string_literal: true

require "minitest/autorun"
require "tuxedo/r"

class TuxedoRTest < Minitest::Test
  def test_version_is_set
    refute_nil ::Tuxedo::R::VERSION
  end

  def test_platform_returns_a_string
    assert_kind_of String, Tuxedo::R.platform
  end

  def test_raises_when_install_dir_missing
    ENV["TUXEDO_INSTALL_DIR"] = "/path/does/not/exist"
    assert_raises(Tuxedo::R::DirectoryNotFoundException) do
      Tuxedo::R.executable
    end
  ensure
    ENV.delete("TUXEDO_INSTALL_DIR")
  end
end
