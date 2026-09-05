class Foo
  def ==(other)
    true
  end

  alias eql? ==
end

alias :new_sym :old_sym

alias $new_global $old_global
