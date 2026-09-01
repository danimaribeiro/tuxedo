config[:shell] ||= Thor::Base.shell.new
config[:shell] &&= Thor::Base.shell.new
config[:shell] += 1

class_eval "def #{name}(*); super end", __FILE__, __LINE__

class Thor
  class << self
    def deprecation_warning(message)
      warn message
    end
  end
end
