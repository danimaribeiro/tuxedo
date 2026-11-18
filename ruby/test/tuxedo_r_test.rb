# frozen_string_literal: true

require "minitest/autorun"
require "tmpdir"
require "fileutils"
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

  def test_install_dir_finds_tuxedo
    assert_install_dir_finds("tuxedo", windows: false)
  end

  def test_install_dir_finds_tuxedo_exe_on_windows
    assert_install_dir_finds("tuxedo.exe", windows: true)
  end

  private

  def assert_install_dir_finds(name, windows:)
    Dir.mktmpdir do |dir|
      expected = File.join(File.realpath(dir), name)
      FileUtils.touch(expected)
      ENV["TUXEDO_INSTALL_DIR"] = File.realpath(dir)
      found = nil
      on_windows(windows) { capture_io { found = Tuxedo::R.executable } }
      assert_equal expected, found
    end
  ensure
    ENV.delete("TUXEDO_INSTALL_DIR")
  end

  # Minitest 6 no longer ships `stub`, so swap the method by hand.
  def on_windows(value)
    original = Gem.method(:win_platform?)
    Gem.singleton_class.send(:remove_method, :win_platform?)
    Gem.singleton_class.send(:define_method, :win_platform?) { value }
    yield
  ensure
    Gem.singleton_class.send(:remove_method, :win_platform?)
    Gem.singleton_class.send(:define_method, :win_platform?, original)
  end
end
